//! `GPUCanvasContext`: the swap chain of a `<Canvas>`, by context id.
//!
//! As react-native-webgpu's `SurfaceInfo` (`cpp/rnwgpu/SurfaceRegistry.h`),
//! a context lives apart from its view: TSX allocates the id, the platform
//! view attaches its surface under it when it is made ([`views`]), and the
//! context renders whether or not a surface is there yet.
//!
//! - **Configure** keeps the configuration and configures the attached
//!   surface, if any. The drawing buffer is the canvas's `width` × `height`
//!   (TSX passes them), not the view's size; a change of them reconfigures
//!   at the next `getCurrentTexture`, as on the web.
//! - **getCurrentTexture** acquires the surface's next texture, or, with no
//!   surface (not attached yet, a headless run, a view that went), an
//!   offscreen texture of the same format and size, so a render loop keeps
//!   working. The same texture id comes back until the frame ends: at
//!   `present()`, `configure()`, `unconfigure()`, or a resize.
//! - **present** presents a surface texture. An offscreen frame is kept:
//!   when a surface attaches later, the platform file calls
//!   [`surface_attached`], which configures the surface and shows that
//!   frame at once (the reference's blit on attach), so a canvas drawn once
//!   before its view was ready still shows its frame.
//!
//! **Where a frame goes**, by what the platform registered for the context
//! id: a surface in [`views`] (Apple, Android, Windows) is presented; with
//! none, the frame is rendered into the context's own offscreen texture,
//! which a presenting view in [`offscreen`] (Linux: [`offscreen::show`])
//! gets read back at each `present()` ([`offscreen::deliver`]), and which a
//! surface attaching later shows. `views.rs` holds surfaces only: a
//! presenting view has no `wgpu::Surface`, so it stays in `offscreen.rs`,
//! beside the readback it needs, and this file asks both.
//!
//! The platform files report what [`views`] does not hold with
//! [`set_native`]: the layer's pointer (`getNativeSurface().surface`), the
//! view's size in points and its scale. See docs/architecture.md "Canvas".
//!
//! [`views`]: crate::surface::views
//! [`offscreen`]: crate::surface::offscreen

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU32, AtomicUsize, Ordering},
    },
};

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;
use serde_json::json;

use crate::{
    api::{device::DeviceEntry, texture::TextureEntry, texture_view::expire_frames},
    convert::parse,
    error::{ErrorName, GpuErrorKind, NativeError, guard},
    registry::{self, Id},
    surface::{offscreen, views},
};

/// `alphaMode: "premultiplied"`. wgpu's Metal surfaces offer `Opaque` and
/// `PostMultiplied` only, where `PostMultiplied` is a non-opaque
/// `CAMetalLayer`, which Core Animation composites as premultiplied: what
/// the spec's (and Dawn's) "premultiplied" is on Apple.
#[cfg(target_vendor = "apple")]
const PREMULTIPLIED: wgpu::CompositeAlphaMode = wgpu::CompositeAlphaMode::PostMultiplied;
#[cfg(not(target_vendor = "apple"))]
const PREMULTIPLIED: wgpu::CompositeAlphaMode = wgpu::CompositeAlphaMode::PreMultiplied;

/// The texture usage bits WebGPU defines (`GPUTextureUsage`).
const SPEC_USAGE: u32 = 0x1f;

/// `GPUTextureUsage.RENDER_ATTACHMENT`, the default usage.
const RENDER_ATTACHMENT: u32 = 0x10;

/// `GPUCanvasConfiguration`, as TSX sends it (`wire`d: the device is its id),
/// with the canvas's drawing buffer size.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Configuration {
    device: Id,
    format: wgpu::TextureFormat,
    #[serde(default = "render_attachment")]
    usage: u32,
    #[serde(default)]
    view_formats: Vec<wgpu::TextureFormat>,
    #[serde(default)]
    color_space: Option<String>,
    #[serde(default)]
    tone_mapping: Option<ToneMapping>,
    #[serde(default)]
    alpha_mode: Option<String>,
    width: u32,
    height: u32,
}

/// `GPUCanvasToneMapping`.
#[derive(Debug, Deserialize)]
struct ToneMapping {
    #[serde(default)]
    mode: Option<String>,
}

fn render_attachment() -> u32 {
    RENDER_ATTACHMENT
}

/// A context's configuration, as wgpu takes it.
#[derive(Clone)]
struct Config {
    device_id: Id,
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// The device's adapter, for the surface's capabilities.
    adapter: Option<wgpu::Adapter>,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    view_formats: Vec<wgpu::TextureFormat>,
    alpha_mode: wgpu::CompositeAlphaMode,
    width: u32,
    height: u32,
    /// What shows the frames on a surface that cannot show `format` as a
    /// canvas means it ([`shown_as`]); `None`: the surface is of `format`.
    shown: Option<Shown>,
}

/// Frames rendered offscreen in the canvas's format and drawn onto a
/// surface of another.
#[derive(Clone)]
struct Shown {
    format: wgpu::TextureFormat,
    blitter: Arc<wgpu::util::TextureBlitter>,
}

/// The surface's format for a canvas of `format` that the platform would
/// show wrongly. Windows takes a 16-bit float swap chain's values as linear
/// light (scRGB), and a canvas's are sRGB-encoded: such a canvas shows in 10
/// bits, or in 8 when it is transparent (10 bits leave 2 for alpha).
fn shown_as(
    format: wgpu::TextureFormat,
    alpha_mode: wgpu::CompositeAlphaMode,
) -> Option<wgpu::TextureFormat> {
    if !cfg!(windows) || format != wgpu::TextureFormat::Rgba16Float {
        return None;
    }
    Some(if alpha_mode == wgpu::CompositeAlphaMode::Opaque {
        wgpu::TextureFormat::Rgb10a2Unorm
    } else {
        wgpu::TextureFormat::Bgra8Unorm
    })
}

impl Config {
    /// The configuration of `surface`, with `extra` usage (`COPY_DST` for
    /// the blit on attach). An `alphaMode` the surface cannot do is the one
    /// it can: opaque when offered, else its first. react-native-webgpu
    /// honours `alphaMode` on Apple only; Android's GL surfaces, for one,
    /// offer `Opaque` alone.
    fn surface(
        &self,
        surface: &wgpu::Surface<'static>,
        extra: wgpu::TextureUsages,
    ) -> wgpu::SurfaceConfiguration {
        let caps = self.adapter.as_ref().map(|a| surface.get_capabilities(a));
        let offered = caps
            .as_ref()
            .map(|c| c.alpha_modes.clone())
            .unwrap_or_default();
        if let Some(shown) = &self.shown {
            return wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: shown.format,
                width: self.width,
                height: self.height,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: alpha_mode(self.alpha_mode, &offered),
                view_formats: Vec::new(),
            };
        }
        let mut usage = self.usage | extra;
        // A usage the spec allows a canvas but this surface lacks (Android's
        // GL surfaces: render attachment only) is dropped, with a validation
        // error naming it, so the canvas still renders; using the texture
        // that way then fails as any missing usage does.
        if let Some(caps) = &caps
            && !caps.usages.contains(usage)
        {
            let lacking = self.usage - caps.usages;
            usage &= caps.usages;
            if !lacking.is_empty()
                && let Ok(entry) = DeviceEntry::get(self.device_id)
            {
                entry.report(
                    GpuErrorKind::Validation,
                    format!("configure: this canvas's surface cannot be used as {lacking:?}; the canvas renders without it"),
                );
            }
        }
        wgpu::SurfaceConfiguration {
            usage,
            format: self.format,
            width: self.width,
            height: self.height,
            // The reference always presents with Fifo (GPUCanvasContext.cpp).
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: alpha_mode(self.alpha_mode, &offered),
            view_formats: self.view_formats.clone(),
        }
    }

    /// An offscreen texture standing in for the surface's.
    fn offscreen(&self) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("canvas (offscreen)"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            // COPY_SRC for the blit onto a surface that attaches later,
            // TEXTURE_BINDING for the one that converts.
            usage: self.usage
                | wgpu::TextureUsages::COPY_SRC
                | if self.shown.is_some() {
                    wgpu::TextureUsages::TEXTURE_BINDING
                } else {
                    wgpu::TextureUsages::empty()
                },
            view_formats: &self.view_formats,
        })
    }
}

/// `requested` when the surface offers it (or nothing is known of what it
/// offers), else `Opaque` when offered, else the first mode offered.
fn alpha_mode(
    requested: wgpu::CompositeAlphaMode,
    offered: &[wgpu::CompositeAlphaMode],
) -> wgpu::CompositeAlphaMode {
    if offered.is_empty() || offered.contains(&requested) {
        requested
    } else if offered.contains(&wgpu::CompositeAlphaMode::Opaque) {
        wgpu::CompositeAlphaMode::Opaque
    } else {
        offered[0]
    }
}

/// Where the current frame's texture came from.
enum Source {
    /// The surface's: presented by `present()`.
    Surface(SurfaceFrame),
    /// The offscreen texture.
    Offscreen,
}

/// A texture acquired from a surface, and that surface. Fields drop in
/// order: the texture is discarded while its surface still exists. wgpu-core
/// panics when a `SurfaceTexture` outlives its surface ("Surface[..] does
/// not exist"), and a panic on the main thread inside an Objective-C
/// `dealloc` aborts the app, so the frame keeps its surface alive.
struct SurfaceFrame {
    texture: wgpu::SurfaceTexture,
    surface: Arc<wgpu::Surface<'static>>,
}

/// The frame in flight: the texture `getCurrentTexture` returned.
struct Frame {
    /// Its id in the registry (TSX's `GPUTexture`).
    id: Id,
    source: Source,
}

/// One canvas's state, as the reference's `SurfaceInfo`: it outlives the
/// `<Canvas>` and its view while a `GPUCanvasContext` object holds it, and
/// renders offscreen once the view went.
#[derive(Default)]
struct Context {
    /// First, so it drops before the surface below (fields drop in order).
    frame: Option<Frame>,
    config: Option<Config>,
    /// The surface last configured, to tell when another one attached.
    configured: Option<Arc<wgpu::Surface<'static>>>,
    /// The offscreen texture (no surface, or none yet).
    offscreen: Option<wgpu::Texture>,
    /// Whether the offscreen texture holds a presented frame not shown on
    /// a surface yet.
    offscreen_presented: bool,
    /// `destroyContext` was called (the `<Canvas>` unmounted): the entry goes
    /// once its view went and no context object holds it.
    destroyed: bool,
    /// The `GPUCanvasContext` objects alive for it ([`WgpuCanvasContext::make`]
    /// adds one, [`WgpuCanvasContext::release`] takes one away when the
    /// object is collected).
    holders: u32,
}

impl Context {
    /// Ends the frame in flight: an unpresented surface texture is dropped
    /// (wgpu discards it).
    fn end_frame(&mut self) -> Option<Frame> {
        self.frame.take()
    }

    /// Whether nothing needs it any more: its `<Canvas>` unmounted, no
    /// context object holds it, and no view is attached under `id`.
    fn retired(&self, id: u32) -> bool {
        self.destroyed && self.holders == 0 && views::get(id).is_none()
    }
}

/// What a platform view tells about itself ([`set_native`]).
#[derive(Debug, Clone, Copy, Default)]
struct Native {
    /// The layer or window (`CAMetalLayer *`), for `getNativeSurface`.
    pointer: usize,
    /// The view's size in points.
    width: f64,
    height: f64,
    /// Device pixels per point.
    scale: f64,
}

static CONTEXTS: Mutex<BTreeMap<u32, Context>> = Mutex::new(BTreeMap::new());
/// The context ids whose `<Canvas>` unmounted (`destroyContext`): a later
/// `make` of one is refused rather than re-creating state that nothing
/// would retire. A few bytes per canvas mounted in the process.
static DESTROYED: Mutex<BTreeSet<u32>> = Mutex::new(BTreeSet::new());
static NATIVE: Mutex<BTreeMap<u32, Native>> = Mutex::new(BTreeMap::new());
/// Context ids: unique for the process (the views outlive a JS reload).
static NEXT_CONTEXT: AtomicU32 = AtomicU32::new(1);
/// View teardowns that panicked ([`view_gone`] catches them).
static TEARDOWN_PANICS: AtomicUsize = AtomicUsize::new(0);

fn contexts() -> MutexGuard<'static, BTreeMap<u32, Context>> {
    CONTEXTS.lock().unwrap_or_else(|e| e.into_inner())
}

fn destroyed() -> MutexGuard<'static, BTreeSet<u32>> {
    DESTROYED.lock().unwrap_or_else(|e| e.into_inner())
}

fn natives() -> MutexGuard<'static, BTreeMap<u32, Native>> {
    NATIVE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The view of `context_id` is a `width` × `height` point view at `scale`
/// on `pointer` (its `CAMetalLayer`, `ANativeWindow`, ...). Platform files
/// call it when the view is made and when its size or scale changes,
/// beside [`views::attach`] / [`views::resize`].
pub fn set_native(context_id: u32, pointer: usize, width: f64, height: f64, scale: f64) {
    natives().insert(
        context_id,
        Native {
            pointer,
            width,
            height,
            scale,
        },
    );
}

/// A surface was attached for `context_id` ([`views::attach`] first). On a
/// configured context with no frame in flight, configures it and, when an
/// offscreen frame was presented meanwhile, shows that frame on it. Called
/// on the platform's UI thread.
pub fn surface_attached(context_id: u32) {
    let Some(view) = views::get(context_id) else {
        return;
    };
    let result = guard(|| {
        let mut all = contexts();
        let Some(ctx) = all.get_mut(&context_id) else {
            return Ok(());
        };
        let Some(config) = ctx.config.clone() else {
            return Ok(());
        };
        if ctx.frame.is_some() {
            // Adopted at the frame's end instead (getCurrentTexture).
            return Ok(());
        }
        let blit = ctx.offscreen.clone().filter(|_| ctx.offscreen_presented);
        let extra = if blit.is_some() {
            wgpu::TextureUsages::COPY_DST
        } else {
            wgpu::TextureUsages::empty()
        };
        configure_view(context_id, &view.surface, &config, extra);
        ctx.configured = Some(Arc::clone(&view.surface));
        ctx.offscreen_presented = false;
        drop(all);
        if let Some(offscreen) = blit {
            blit_and_present(context_id, &view.surface, &config, &offscreen);
        }
        Ok(())
    });
    if let Err(error) = result {
        log_error(context_id, &error);
    }
}

/// The view of `context_id` went: every platform file calls this from the
/// view's teardown (on the UI thread), in place of [`views::detach`].
///
/// In order, as the reference's `SurfaceInfo::detach`: the frame in flight
/// is ended (an acquired, unpresented surface texture is discarded while its
/// surface still exists), the surface is detached and dropped, and the
/// context renders offscreen from then on, keeping its configuration, so a
/// render loop that outlives its `<Canvas>` keeps working. The context
/// itself goes only when its `<Canvas>` unmounted and no context object
/// holds it. Never panics: a panic here would unwind into a platform
/// `dealloc` / destructor and abort the app; it is logged instead.
pub fn view_gone(context_id: u32) {
    let result = std::panic::catch_unwind(|| detach_view(context_id));
    if let Err(panic) = result {
        TEARDOWN_PANICS.fetch_add(1, Ordering::Relaxed);
        let what = panic
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        log_error(context_id, &format!("detaching its view panicked: {what}"));
    }
}

fn detach_view(context_id: u32) {
    natives().remove(&context_id);
    // Under the contexts lock (the order everywhere: contexts, then views),
    // so a frame the JS thread acquires meanwhile sees the view gone.
    let (frame, view, gone) = {
        let mut all = contexts();
        let frame = all.get_mut(&context_id).and_then(|ctx| {
            ctx.configured = None;
            // Only a surface frame: an offscreen one stays valid.
            match ctx.frame {
                Some(Frame {
                    source: Source::Surface(_),
                    ..
                }) => ctx.end_frame(),
                _ => None,
            }
        });
        let view = views::take(context_id);
        let gone = match all.get(&context_id) {
            Some(ctx) if ctx.retired(context_id) => all.remove(&context_id),
            _ => None,
        };
        (frame, view, gone)
    };
    // The texture, then its surface.
    drop(frame);
    drop(gone);
    drop(view);
}

fn log_error(context_id: u32, error: &str) {
    eprintln!("nsh-webgpu: canvas {context_id}: {error}");
}

/// Configures `surface`, the one of `context_id`'s view, with `config`.
fn configure_view(
    context_id: u32,
    surface: &Arc<wgpu::Surface<'static>>,
    config: &Config,
    extra: wgpu::TextureUsages,
) {
    reconfigure(context_id, surface, config, extra);
    // A composition visual shows its frame 1:1: it scales to the view.
    #[cfg(windows)]
    crate::surface::windows::buffer_size(context_id, config.width, config.height);
}

/// [`configure_view`], of the same size.
fn reconfigure(
    context_id: u32,
    surface: &Arc<wgpu::Surface<'static>>,
    config: &Config,
    extra: wgpu::TextureUsages,
) {
    expire_frames(context_id);
    if let Some(before) = views::configured_with(context_id, surface, &config.device) {
        release_swap_chain(surface, &before);
    }
    surface.configure(&config.device, &config.surface(surface, extra));
}

/// `surface` lets go of the swap chain it has for `device`: the next
/// configure is another device's. A swap chain belongs to the device that
/// made it (on DX12, to its queue), and wgpu's `configure` keeps the one it
/// has: the other device would then draw into buffers that are not its own,
/// for which Windows removes the GPU device, every `GPUDevice` with it.
/// Metal's layer has no swap chain of a device.
fn release_swap_chain(surface: &wgpu::Surface<'static>, device: &wgpu::Device) {
    #[cfg(any(windows, target_os = "android"))]
    {
        use wgpu::hal::{Api, Surface as _, api};

        fn release<A: Api>(surface: &wgpu::Surface<'static>, device: &wgpu::Device) {
            // SAFETY: neither is destroyed here.
            let (surface, device) = unsafe { (surface.as_hal::<A>(), device.as_hal::<A>()) };
            if let (Some(surface), Some(device)) = (surface, device) {
                // SAFETY: the device the surface was configured with; its
                // work is done (the poll below) and its frames expired.
                unsafe { surface.unconfigure(&device) };
            }
        }
        // A lost device has nothing to wait for; a hung one is not waited for.
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(5)),
        });
        #[cfg(windows)]
        release::<api::Dx12>(surface, device);
        release::<api::Vulkan>(surface, device);
        release::<api::Gles>(surface, device);
    }
    #[cfg(not(any(windows, target_os = "android")))]
    let _ = (surface, device);
}

/// Copies `offscreen` onto the surface's next texture and presents it.
fn blit_and_present(
    context_id: u32,
    surface: &Arc<wgpu::Surface<'static>>,
    config: &Config,
    offscreen: &wgpu::Texture,
) {
    let Some(texture) = acquire(context_id, surface, config) else {
        return;
    };
    if texture.texture.size() != offscreen.size() {
        return;
    }
    // The surface may lack COPY_DST (Android's GL surfaces): nothing to show then.
    if config.shown.is_none()
        && !texture
            .texture
            .usage()
            .contains(wgpu::TextureUsages::COPY_DST)
    {
        return;
    }
    let mut encoder = config
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("canvas blit"),
        });
    match &config.shown {
        Some(shown) => show(
            shown,
            &config.device,
            &mut encoder,
            offscreen,
            &texture.texture,
        ),
        None => encoder.copy_texture_to_texture(
            offscreen.as_image_copy(),
            texture.texture.as_image_copy(),
            offscreen.size(),
        ),
    }
    config.queue.submit([encoder.finish()]);
    texture.present();
}

/// Draws `frame` onto `target`, of `shown`'s format.
fn show(
    shown: &Shown,
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    frame: &wgpu::Texture,
    target: &wgpu::Texture,
) {
    let view = wgpu::TextureViewDescriptor::default();
    shown.blitter.copy(
        device,
        encoder,
        &frame.create_view(&view),
        &target.create_view(&view),
    );
}

/// The texture usages the spec allows a canvas, minus what the surface
/// cannot do: the usage to configure with, reporting a validation error for
/// what it drops.
fn checked_usage(entry: &DeviceEntry, usage: u32) -> wgpu::TextureUsages {
    if usage & !SPEC_USAGE != 0 {
        entry.report(
            GpuErrorKind::Validation,
            format!("configure: usage 0x{usage:x} has bits WebGPU does not define"),
        );
        return wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    wgpu::TextureUsages::from_bits_truncate(usage)
}

/// The formats a canvas may be configured with (the spec's "supported
/// context formats").
fn canvas_format(format: wgpu::TextureFormat) -> bool {
    matches!(
        format,
        wgpu::TextureFormat::Bgra8Unorm
            | wgpu::TextureFormat::Rgba8Unorm
            | wgpu::TextureFormat::Rgba16Float
    )
}

/// The native half of `GPUCanvasContext` (and of the `RNWebGPU` canvas
/// functions: `allocateContextId`, `destroyContext`, `getNativeSurface`).
#[module]
pub struct WgpuCanvasContext;

#[module]
impl WgpuCanvasContext {
    /// A context id no other canvas of the process has had.
    pub fn allocate_context_id() -> u32 {
        NEXT_CONTEXT.fetch_add(1, Ordering::Relaxed)
    }

    /// `getContext("webgpu")` on canvas `context` made a context object:
    /// its state, made on first use (several context objects of one canvas
    /// share it), holds one more object. [`Self::release`] when the object
    /// is collected.
    ///
    /// An `InvalidStateError` once the canvas was destroyed
    /// ([`Self::destroy_context`]): its state is not made again.
    pub fn make(context: u32) -> Result<(), String> {
        if destroyed().contains(&context) {
            return Err(NativeError::new(
                ErrorName::InvalidStateError,
                format!("canvas {context} was destroyed (its <Canvas> unmounted)"),
            )
            .into());
        }
        let mut all = contexts();
        let ctx = all.entry(context).or_default();
        ctx.holders = ctx.holders.saturating_add(1);
        Ok(())
    }

    /// A context object of `context` was collected (TSX's finalizer): the
    /// state goes when it was the last one, its `<Canvas>` unmounted and no
    /// view is attached.
    pub fn release(context: u32) {
        let gone = {
            let mut all = contexts();
            match all.get_mut(&context) {
                Some(ctx) => {
                    ctx.holders = ctx.holders.saturating_sub(1);
                    if ctx.retired(context) {
                        all.remove(&context)
                    } else {
                        None
                    }
                }
                None => None,
            }
        };
        drop(gone);
    }

    /// The `<Canvas>` of `context` unmounted (`RNWebGPU.destroyContext`).
    /// Its state stays while a view is attached (the view's teardown,
    /// [`view_gone`], retires it) or a context object holds it, which keeps
    /// rendering, offscreen once the view went, as the reference's context
    /// does; it goes now when neither does.
    pub fn destroy_context(context: u32) {
        destroyed().insert(context);
        let gone = {
            let mut all = contexts();
            match all.get_mut(&context) {
                Some(ctx) => {
                    ctx.destroyed = true;
                    if ctx.retired(context) {
                        all.remove(&context)
                    } else {
                        None
                    }
                }
                None => None,
            }
        };
        drop(gone);
    }

    /// `getNativeSurface()`: `{ surface, width, height, scale }`, the
    /// layer's pointer as a decimal string (TSX makes a BigInt of it) and the
    /// view's size in points; zeros before the view attached.
    pub fn native_surface(context: u32) -> JsonValue {
        let native = natives().get(&context).copied().unwrap_or_default();
        json!({
            "surface": native.pointer.to_string(),
            "width": native.width,
            "height": native.height,
            "scale": native.scale,
        })
    }

    /// `context.configure(configuration)`. Throws a `TypeError` for a
    /// format a canvas does not take, an unknown `colorSpace`, `toneMapping`
    /// or `alphaMode`; invalid usage is a validation error of the device.
    pub fn configure(context: u32, configuration: JsonValue) -> Result<(), String> {
        guard(|| {
            let c: Configuration = parse("GPUCanvasConfiguration", configuration)?;
            if let Some(space) = c.color_space.as_deref()
                && !matches!(space, "srgb" | "display-p3")
            {
                return Err(NativeError::type_error(format!(
                    "'{space}' is not a valid value for enumeration PredefinedColorSpace"
                )));
            }
            if let Some(mode) = c.tone_mapping.as_ref().and_then(|t| t.mode.as_deref())
                && !matches!(mode, "standard" | "extended")
            {
                return Err(NativeError::type_error(format!(
                    "'{mode}' is not a valid value for enumeration GPUCanvasToneMappingMode"
                )));
            }
            let alpha_mode = match c.alpha_mode.as_deref() {
                None | Some("opaque") => wgpu::CompositeAlphaMode::Opaque,
                Some("premultiplied") => PREMULTIPLIED,
                Some(other) => {
                    return Err(NativeError::type_error(format!(
                        "'{other}' is not a valid value for enumeration GPUCanvasAlphaMode"
                    )));
                }
            };
            if !canvas_format(c.format) {
                return Err(NativeError::type_error(format!(
                    "configure: {:?} is not a format a canvas supports (bgra8unorm, rgba8unorm, rgba16float)",
                    c.format
                )));
            }
            let entry = DeviceEntry::get(c.device)?;
            let usage = checked_usage(&entry, c.usage);
            let shown = shown_as(c.format, alpha_mode).map(|format| Shown {
                format,
                blitter: Arc::new(wgpu::util::TextureBlitter::new(&entry.device, format)),
            });
            let config = Config {
                device_id: c.device,
                device: entry.device.clone(),
                queue: entry.queue.clone(),
                adapter: entry.adapter.clone(),
                format: c.format,
                usage,
                view_formats: c.view_formats,
                alpha_mode,
                width: c.width.max(1),
                height: c.height.max(1),
                shown,
            };
            let view = views::get(context);
            let ended = {
                let mut all = contexts();
                let ctx = all.entry(context).or_default();
                let ended = ctx.end_frame();
                ctx.offscreen = None;
                ctx.offscreen_presented = false;
                ctx.config = Some(config.clone());
                ctx.configured = view.as_ref().map(|v| Arc::clone(&v.surface));
                ended
            };
            drop(ended);
            if let Some(view) = view {
                configure_view(
                    context,
                    &view.surface,
                    &config,
                    wgpu::TextureUsages::empty(),
                );
            }
            Ok(())
        })
    }

    /// `context.unconfigure()`: the configuration and the frame go.
    pub fn unconfigure(context: u32) -> Result<(), String> {
        guard(|| {
            let ended = {
                let mut all = contexts();
                let ctx = all.entry(context).or_default();
                ctx.config = None;
                ctx.offscreen = None;
                ctx.offscreen_presented = false;
                ctx.configured = None;
                ctx.end_frame()
            };
            drop(ended);
            Ok(())
        })
    }

    /// `context.getCurrentTexture()` for a `width` × `height` drawing
    /// buffer: the id of the frame's texture, the same until the frame
    /// ends. An `InvalidStateError` before `configure()`.
    pub fn get_current_texture(context: u32, width: u32, height: u32) -> Result<u32, String> {
        guard(|| current_texture(context, width.max(1), height.max(1)))
    }

    /// `context.present()`: shows the frame (a surface's). An offscreen
    /// frame goes to the context's presenting view when it has one
    /// (Linux: [`offscreen::deliver`]), and is kept for a surface that
    /// attaches later. Ends the frame.
    pub fn present(context: u32) -> Result<(), String> {
        guard(|| {
            let (frame, offscreen) = {
                let mut all = contexts();
                let Some(ctx) = all.get_mut(&context) else {
                    return Ok(());
                };
                let frame = ctx.end_frame();
                let mut offscreen = None;
                if matches!(
                    frame,
                    Some(Frame {
                        source: Source::Offscreen,
                        ..
                    })
                ) {
                    ctx.offscreen_presented = true;
                    offscreen = ctx.offscreen.clone().zip(ctx.config.clone());
                }
                (frame, offscreen)
            };
            if let Some(Frame {
                source: Source::Surface(SurfaceFrame { texture, surface }),
                ..
            }) = frame
            {
                // Presented while its surface is held.
                texture.present();
                drop(surface);
            }
            if let Some((texture, config)) = offscreen {
                // A frame that never goes to its surface as it is.
                if config.shown.is_some()
                    && let Some(view) = views::get(context)
                {
                    blit_and_present(context, &view.surface, &config, &texture);
                    return Ok(());
                }
                let device = DeviceEntry::get(config.device_id)?;
                offscreen::deliver(
                    context,
                    &device,
                    &texture,
                    config.alpha_mode == wgpu::CompositeAlphaMode::Opaque,
                )?;
            }
            Ok(())
        })
    }

    /// Whether `context` presents to a surface now (tests and diagnostics).
    pub fn has_surface(context: u32) -> bool {
        views::get(context).is_some()
    }
}

fn current_texture(context: u32, width: u32, height: u32) -> Result<u32, NativeError> {
    let view = views::get(context);
    let (config, ended, configure_surface) = {
        let mut all = contexts();
        let ctx = all.entry(context).or_default();
        let Some(config) = ctx.config.as_mut() else {
            return Err(NativeError::new(
                ErrorName::InvalidStateError,
                "getCurrentTexture() called on a canvas context that is not configured; call context.configure() first",
            ));
        };
        let mut ended = None;
        let mut resized = false;
        if config.width != width || config.height != height {
            // The drawing buffer follows canvas.width/height: a new size ends
            // the frame and reconfigures.
            config.width = width;
            config.height = height;
            resized = true;
            ended = ctx.end_frame();
            ctx.offscreen = None;
            ctx.offscreen_presented = false;
        }
        if let Some(frame) = &ctx.frame {
            return Ok(frame.id);
        }
        let config = ctx.config.clone().expect("checked above");
        let configure_surface = match &view {
            // A surface attached since the last frame, or the size changed:
            // (re)configure it at this frame boundary.
            Some(v) => {
                let new = !ctx
                    .configured
                    .as_ref()
                    .is_some_and(|c| Arc::ptr_eq(c, &v.surface));
                ctx.configured = Some(Arc::clone(&v.surface));
                new || resized
            }
            None => {
                ctx.configured = None;
                false
            }
        };
        (config, ended, configure_surface)
    };
    drop(ended);
    let surface = view.map(|v| v.surface);
    if let (Some(surface), true) = (&surface, configure_surface) {
        configure_view(context, surface, &config, wgpu::TextureUsages::empty());
    }
    let acquired = surface
        .as_ref()
        .filter(|_| config.shown.is_none())
        .and_then(|s| {
            acquire(context, s, &config).map(|texture| SurfaceFrame {
                texture,
                surface: Arc::clone(s),
            })
        });
    let mut all = contexts();
    // The view may have gone while the texture was acquired (the UI thread
    // tears it down without waiting for the JS thread): then the texture is
    // discarded, its surface still alive, and this frame renders offscreen.
    let acquired = acquired.filter(|frame| {
        views::get(context).is_some_and(|v| Arc::ptr_eq(&v.surface, &frame.surface))
    });
    let ctx = all.entry(context).or_default();
    let (texture, source) = match acquired {
        Some(frame) => (frame.texture.texture.clone(), Source::Surface(frame)),
        None => {
            let texture = ctx
                .offscreen
                .get_or_insert_with(|| config.offscreen())
                .clone();
            ctx.offscreen_presented = false;
            (texture, Source::Offscreen)
        }
    };
    let frame_of = matches!(source, Source::Surface(_)).then_some(context);
    let id = registry::insert(TextureEntry {
        texture,
        device: config.device_id,
        frame_of,
    });
    ctx.frame = Some(Frame { id, source });
    Ok(id)
}

/// The surface's next texture; once more after reconfiguring when it is
/// outdated or lost; `None` (render offscreen) when it times out or the
/// window is occluded.
fn acquire(
    context_id: u32,
    surface: &Arc<wgpu::Surface<'static>>,
    config: &Config,
) -> Option<wgpu::SurfaceTexture> {
    for attempt in 0..2 {
        match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                return Some(t);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost
                if attempt == 0 =>
            {
                reconfigure(context_id, surface, config, wgpu::TextureUsages::empty());
            }
            _ => return None,
        }
    }
    None
}

/// How many view teardowns ([`view_gone`]) panicked and were caught
/// (tests check there were none).
pub fn teardown_panics() -> usize {
    TEARDOWN_PANICS.load(Ordering::Relaxed)
}

/// How many contexts are alive (tests check `destroyContext`).
pub fn context_count() -> usize {
    contexts().len()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{Shown, alpha_mode, show, shown_as};
    use wgpu::CompositeAlphaMode as A;

    /// A 16-bit float canvas's values are sRGB-encoded, as any canvas's:
    /// they show as they are, not as linear light (Windows showed 55 as 128).
    #[test]
    fn a_float_canvas_shows_its_values_as_they_are() {
        use wgpu::TextureFormat as F;

        assert_eq!(shown_as(F::Bgra8Unorm, A::Opaque), None);
        let windows = |format| cfg!(windows).then_some(format);
        assert_eq!(
            shown_as(F::Rgba16Float, A::Opaque),
            windows(F::Rgb10a2Unorm)
        );
        assert_eq!(
            shown_as(F::Rgba16Float, A::PreMultiplied),
            windows(F::Bgra8Unorm)
        );

        let adapter = crate::instance::request_adapter(&Default::default()).expect("an adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("a device");
        let texture = |format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let target_format = F::Rgba8Unorm;
        let shown = Shown {
            format: target_format,
            blitter: Arc::new(wgpu::util::TextureBlitter::new(&device, target_format)),
        };
        let frame = texture(
            F::Rgba16Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let target = texture(
            target_format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let view = frame.create_view(&wgpu::TextureViewDescriptor::default());
        drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 55.0 / 255.0,
                        g: 0.5,
                        b: 2.0,
                        a: 0.25,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        }));
        show(&shown, &device, &mut encoder, &frame, &target);
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: None,
                },
            },
            target.size(),
        );
        queue.submit([encoder.finish()]);
        readback.slice(..).map_async(wgpu::MapMode::Read, |_| ());
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the frame");
        let pixel: Vec<i32> = readback.slice(..4).get_mapped_range()[..]
            .iter()
            .map(|b| i32::from(*b))
            .collect();
        for (got, want) in pixel.iter().zip([55, 128, 255, 64]) {
            assert!((got - want).abs() <= 1, "{pixel:?}");
        }
    }

    #[test]
    fn an_alpha_mode_the_surface_lacks_falls_back() {
        // Android's GL surfaces: opaque only.
        assert_eq!(alpha_mode(A::PreMultiplied, &[A::Opaque]), A::Opaque);
        assert_eq!(alpha_mode(A::PreMultiplied, &[A::Inherit]), A::Inherit);
        assert_eq!(
            alpha_mode(A::PreMultiplied, &[A::Opaque, A::PreMultiplied]),
            A::PreMultiplied
        );
        // Nothing known (a device without its adapter): as asked.
        assert_eq!(alpha_mode(A::PreMultiplied, &[]), A::PreMultiplied);
    }
}
