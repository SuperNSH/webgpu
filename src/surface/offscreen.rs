//! The offscreen canvas: a context drawn into a texture instead of a
//! platform surface, with readback. Three users:
//!
//! - `GPUOffscreenCanvas` (TSX, area react-parity): `readPixels` /
//!   `takePixels` read its texture back with the 256-byte row alignment
//!   WebGPU's copies need handled here (the reference's `getImageData`
//!   copies with `bytesPerRow = width * 4` and so only works for widths
//!   that are a multiple of 64).
//! - Linux, where a GTK 4 widget has no native window of its own to make a
//!   `wgpu::Surface` on (`linux.rs`): the view registers a [`Presenter`]
//!   and its size with [`show`]. The canvas context (`api/canvas_context.rs`)
//!   renders a context with no surface in [`super::views`] into a texture of
//!   its own, and its `present()` hands that texture to [`deliver`], which
//!   reads it back off the JS thread and gives the presenter its pixels.
//! - Rust callers that want a context with no `<Canvas>` at all:
//!   [`configure`], [`current_texture`], [`present`] and [`unconfigure`]
//!   keep the spec's frame rules (the same texture until `present`/
//!   `configure`/`unconfigure`/a resize, a new one after), and present to a
//!   presenting view too when one is [`show`]n under the id.
//!
//! Everything here is keyed by the canvas's context id, as `views.rs` is.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
};

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;
use serde_json::json;

use crate::{
    api::{
        device::DeviceEntry,
        texture::{TextureEntry, WgpuTexture},
    },
    convert::parse,
    error::{NativeError, guard},
    pending,
    registry::{self, Id},
};

/// A frame read back: tightly packed rows (`width * bytes_per_pixel`), top
/// row first, in `format`'s byte order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The texture's format.
    pub format: wgpu::TextureFormat,
    /// The pixels.
    pub data: Vec<u8>,
    /// The context's `alphaMode` is `"opaque"`: alpha is to be ignored.
    pub opaque: bool,
}

impl Frame {
    /// The pixels as RGBA8 (a BGRA8 frame swizzled); `None` for formats that
    /// are not 8-bit RGBA or BGRA.
    pub fn to_rgba8(&self) -> Option<Vec<u8>> {
        use wgpu::TextureFormat as F;
        match self.format {
            F::Rgba8Unorm | F::Rgba8UnormSrgb => Some(self.data.clone()),
            F::Bgra8Unorm | F::Bgra8UnormSrgb => Some(
                self.data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|&[b, g, r, a]| [r, g, b, a])
                    .collect(),
            ),
            _ => None,
        }
    }
}

/// Where a presented frame goes (Linux's picture widget). Called off the JS
/// thread, on the thread that polled the readback.
pub type Presenter = Arc<dyn Fn(Frame) + Send + Sync>;

/// A view that shows the offscreen frames of a context (`linux.rs`).
#[derive(Clone)]
struct Shown {
    presenter: Presenter,
    width: u32,
    height: u32,
    /// A readback to the presenter is in flight: the next frame skips its
    /// readback (a dropped frame) rather than queueing up.
    in_flight: Arc<Mutex<bool>>,
}

/// A configured offscreen context.
#[derive(Clone)]
struct Target {
    device: Id,
    format: wgpu::TextureFormat,
    usage: u32,
    view_formats: Vec<wgpu::TextureFormat>,
    opaque: bool,
    width: u32,
    height: u32,
    /// This frame's texture (its id and the texture), until `present`.
    current: Option<(Id, wgpu::Texture)>,
}

#[derive(Default)]
struct State {
    targets: BTreeMap<u32, Target>,
    shown: BTreeMap<u32, Shown>,
}

static STATE: Mutex<State> = Mutex::new(State {
    targets: BTreeMap::new(),
    shown: BTreeMap::new(),
});

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

// ---- views shown from offscreen frames (Linux) --------------------------

/// Context `context_id`'s view shows its frames through `presenter`, and is
/// `width` × `height` physical pixels (replacing one shown before).
pub fn show(context_id: u32, presenter: Presenter, width: u32, height: u32) {
    state().shown.insert(
        context_id,
        Shown {
            presenter,
            width,
            height,
            in_flight: Arc::default(),
        },
    );
}

/// Context `context_id`'s presenting view is now `width` × `height`.
pub fn resize(context_id: u32, width: u32, height: u32) {
    if let Some(shown) = state().shown.get_mut(&context_id) {
        shown.width = width;
        shown.height = height;
    }
}

/// Context `context_id`'s presenting view went.
pub fn hide(context_id: u32) {
    let shown = state().shown.remove(&context_id);
    drop(shown);
}

/// The size of context `context_id`'s presenting view, if it has one.
pub fn shown_size(context_id: u32) -> Option<(u32, u32)> {
    state().shown.get(&context_id).map(|s| (s.width, s.height))
}

// ---- the context ----------------------------------------------------------

/// `GPUCanvasConfiguration`, the members an offscreen target uses.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Configuration {
    device: Id,
    format: wgpu::TextureFormat,
    #[serde(default = "render_attachment")]
    usage: u32,
    #[serde(default)]
    view_formats: Vec<wgpu::TextureFormat>,
    #[serde(default = "opaque")]
    alpha_mode: String,
}

fn opaque() -> String {
    "opaque".to_owned()
}

fn render_attachment() -> u32 {
    wgpu::TextureUsages::RENDER_ATTACHMENT.bits()
}

/// `context.configure(configuration)` for an offscreen context of `width`
/// × `height` (a presenting view's size wins when it has one): the next
/// [`current_texture`] makes a texture of that format and usage (plus
/// `COPY_SRC`, for readback).
pub fn configure(
    context_id: u32,
    configuration: JsonValue,
    width: u32,
    height: u32,
) -> Result<(), NativeError> {
    let config: Configuration = parse("GPUCanvasConfiguration", configuration)?;
    DeviceEntry::get(config.device)?;
    let (width, height) = shown_size(context_id).unwrap_or((width, height));
    state().targets.insert(
        context_id,
        Target {
            device: config.device,
            format: config.format,
            usage: config.usage,
            view_formats: config.view_formats,
            opaque: match config.alpha_mode.as_str() {
                "opaque" => true,
                "premultiplied" => false,
                other => {
                    return Err(NativeError::type_error(format!(
                        "GPUCanvasConfiguration: alphaMode {other:?} is not a GPUCanvasAlphaMode"
                    )));
                }
            },
            width: width.max(1),
            height: height.max(1),
            current: None,
        },
    );
    Ok(())
}

/// `context.unconfigure()`: the target and its texture go.
pub fn unconfigure(context_id: u32) {
    let target = state().targets.remove(&context_id);
    if let Some((_, texture)) = target.and_then(|t| t.current) {
        texture.destroy();
    }
}

/// Whether context `context_id` is configured offscreen.
pub fn is_configured(context_id: u32) -> bool {
    state().targets.contains_key(&context_id)
}

/// `context.getCurrentTexture()`: this frame's texture id (a
/// `GPUTexture`'s), made on the first call of the frame. A presenting view
/// that changed size since makes it at the new size.
pub fn current_texture(context_id: u32) -> Result<Id, NativeError> {
    let mut target = state().targets.get(&context_id).cloned().ok_or_else(|| {
        NativeError::new(
            crate::error::ErrorName::InvalidStateError,
            "getCurrentTexture: the context is not configured",
        )
    })?;
    if let Some((w, h)) = shown_size(context_id)
        && (w.max(1), h.max(1)) != (target.width, target.height)
    {
        target.width = w.max(1);
        target.height = h.max(1);
        target.current = None;
    }
    if let Some((id, _)) = &target.current {
        return Ok(*id);
    }
    let usage = target.usage | wgpu::TextureUsages::COPY_SRC.bits();
    let id = WgpuTexture::create(
        target.device,
        json!({
            "label": "GPUCanvasContext texture",
            "size": [target.width, target.height, 1],
            "format": target.format,
            "usage": usage,
            "viewFormats": target.view_formats,
        }),
    )
    .map_err(NativeError::operation)?;
    let texture = registry::get::<TextureEntry>(id)?.texture;
    target.current = Some((id, texture));
    state().targets.insert(context_id, target);
    Ok(id)
}

/// The size of context `context_id`'s textures (its presenting view's, or
/// the size it was configured with).
pub fn size(context_id: u32) -> Option<(u32, u32)> {
    state()
        .targets
        .get(&context_id)
        .map(|t| (t.width, t.height))
}

/// `context.present()`: ends the frame (the next [`current_texture`] is a
/// new texture) and [`deliver`]s it to a presenting view, if one is shown.
/// Without a current texture (nothing drawn) it does nothing.
pub fn present(context_id: u32) -> Result<(), NativeError> {
    let (texture, target) = {
        let mut state = state();
        let Some(target) = state.targets.get_mut(&context_id) else {
            return Ok(());
        };
        let Some((_, texture)) = target.current.take() else {
            return Ok(());
        };
        (texture, target.clone())
    };
    let device = DeviceEntry::get(target.device)?;
    deliver(context_id, &device, &texture, target.opaque)?;
    Ok(())
}

/// Hands the frame in `texture` (mip 0, layer 0; it needs `COPY_SRC`) to
/// context `context_id`'s presenting view: copied to a buffer in this
/// call's submission, read back off the JS thread, the presenter gets its
/// pixels. `opaque`: the context's `alphaMode` is `"opaque"`. A frame
/// delivered while the last one is still being read back is dropped.
/// Whether a presenting view is shown (`false`: nothing to do).
pub fn deliver(
    context_id: u32,
    device: &DeviceEntry,
    texture: &wgpu::Texture,
    opaque: bool,
) -> Result<bool, NativeError> {
    let Some(shown) = state().shown.get(&context_id).cloned() else {
        return Ok(false);
    };
    {
        let mut busy = shown.in_flight.lock().unwrap_or_else(|e| e.into_inner());
        if *busy {
            return Ok(true);
        }
        *busy = true;
    }
    let (readback, ticket) = match Readback::start(device, texture) {
        Ok(started) => started,
        Err(e) => {
            *shown.in_flight.lock().unwrap_or_else(|e| e.into_inner()) = false;
            return Err(e);
        }
    };
    let Shown {
        presenter,
        in_flight,
        ..
    } = shown;
    let poll = device.device.clone();
    let run = move || {
        let frame = readback.finish(&poll).map(|f| Frame { opaque, ..f });
        let _ = pending::check(ticket);
        *in_flight.lock().unwrap_or_else(|e| e.into_inner()) = false;
        if let Ok(frame) = frame {
            presenter(frame);
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(run);
    #[cfg(target_arch = "wasm32")]
    run();
    Ok(true)
}

// ---- readback -------------------------------------------------------------

/// A texture's copy to a mappable buffer, submitted, not yet read.
struct Readback {
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    row: u32,
    padded_row: u32,
    /// The map's outcome, as the ticket's slot gets it (which `Wgpu.wait`
    /// takes; this one stays).
    done: pending::Slot,
}

impl Readback {
    /// Copies mip level 0, layer 0 of `texture` to a new buffer (rows
    /// padded to 256 bytes) and starts mapping it.
    fn start(device: &DeviceEntry, texture: &wgpu::Texture) -> Result<(Self, Id), NativeError> {
        let format = texture.format();
        let bpp = format
            .block_copy_size(None)
            .filter(|_| format.block_dimensions() == (1, 1));
        let Some(bpp) = bpp else {
            return Err(NativeError::operation(format!(
                "readback: format {format:?} cannot be read back"
            )));
        };
        if !texture.usage().contains(wgpu::TextureUsages::COPY_SRC) {
            return Err(NativeError::operation(
                "readback: the texture's usage has no COPY_SRC",
            ));
        }
        let (width, height) = (texture.width(), texture.height());
        let row = width * bpp;
        let padded_row =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("nsh-webgpu readback"),
            size: u64::from(padded_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("nsh-webgpu readback"),
            });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        device.queue.submit([encoder.finish()]);
        let (ticket, slot) = pending::start(&device.device);
        let done = pending::Slot::default();
        let filled = Arc::clone(&done);
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            let outcome = result.map_err(|e| NativeError::operation(format!("readback: {e}")));
            pending::fill(&filled, outcome.clone());
            pending::fill(&slot, outcome);
        });
        Ok((
            Self {
                buffer,
                width,
                height,
                format,
                row,
                padded_row,
                done,
            },
            ticket,
        ))
    }

    /// Whether the map finished (and how).
    fn outcome(&self) -> Option<Result<(), NativeError>> {
        self.done.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// The frame, once mapped (polls `device` until it is).
    fn finish(self, device: &wgpu::Device) -> Result<Frame, NativeError> {
        for _ in 0..8 {
            if self.outcome().is_some() {
                break;
            }
            device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| NativeError::operation(format!("readback: poll: {e}")))?;
        }
        match self.outcome() {
            Some(Ok(())) => {}
            Some(Err(e)) => return Err(e),
            None => return Err(NativeError::operation("readback: the GPU did not finish")),
        }
        Ok(self.unpad())
    }

    /// The mapped rows without their padding.
    fn unpad(self) -> Frame {
        let mapped = self.buffer.slice(..).get_mapped_range();
        let mut data = Vec::with_capacity((self.row * self.height) as usize);
        for y in 0..self.height as usize {
            let start = y * self.padded_row as usize;
            data.extend_from_slice(&mapped[start..start + self.row as usize]);
        }
        drop(mapped);
        self.buffer.unmap();
        Frame {
            width: self.width,
            height: self.height,
            format: self.format,
            data,
            opaque: false,
        }
    }
}

/// Readbacks TSX started, by ticket (wgpu's handles are `Send` off wasm,
/// and on it with `fragile-send-sync-non-atomic-wasm`).
static READBACKS: Mutex<BTreeMap<Id, Readback>> = Mutex::new(BTreeMap::new());

fn readbacks() -> MutexGuard<'static, BTreeMap<Id, Readback>> {
    READBACKS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Reads mip level 0, layer 0 of `texture` (an id) back, blocking until the
/// GPU is done: Rust callers and tests. TSX uses `readPixels` + `Wgpu.wait`
/// + `takePixels`, which do not block the JS thread.
pub fn read_texture(texture: Id) -> Result<Frame, NativeError> {
    let entry = registry::get::<TextureEntry>(texture)?;
    let device = DeviceEntry::get(entry.device)?;
    let (readback, ticket) = Readback::start(&device, &entry.texture)?;
    let frame = readback.finish(&device.device);
    // The ticket's outcome was read through the slot; drop the ticket.
    let _ = pending::check(ticket);
    frame
}

/// The native half of `GPUOffscreenCanvas` and of the offscreen canvas
/// context. Registered in `lib.rs` by the orchestrator (see
/// docs/parity/surfaces-other.md); Rust callers use it directly.
#[module]
pub struct WgpuOffscreenCanvas;

#[module]
impl WgpuOffscreenCanvas {
    /// Starts reading `texture` back (mip 0, layer 0): a ticket for
    /// `Wgpu.wait`, then `takePixels(ticket)`. The texture needs
    /// `COPY_SRC`; any width works (rows are padded and unpadded here).
    pub fn read_pixels(texture: u32) -> Result<u32, String> {
        guard(|| {
            let entry = registry::get::<TextureEntry>(texture)?;
            let device = DeviceEntry::get(entry.device)?;
            let (readback, ticket) = Readback::start(&device, &entry.texture)?;
            readbacks().insert(ticket, readback);
            Ok(ticket)
        })
    }

    /// The pixels of readback `ticket` after `Wgpu.wait(ticket)` settled:
    /// `width * height * bytesPerPixel` bytes, rows top first, in the
    /// texture's own byte order (`bgra8unorm` stays BGRA, as WebGPU's copy
    /// does).
    pub fn take_pixels(ticket: u32) -> Result<Vec<u8>, String> {
        guard(|| {
            let readback = readbacks()
                .remove(&ticket)
                .ok_or_else(|| NativeError::type_error(format!("no readback {ticket}")))?;
            match readback.outcome() {
                Some(Ok(())) => Ok(readback.unpad().data),
                Some(Err(e)) => Err(e),
                None => Err(NativeError::operation(
                    "takePixels: the readback has not finished (await Wgpu.wait first)",
                )),
            }
        })
    }

    /// `configure(configuration)` of an offscreen context `context` of
    /// `width` × `height`.
    pub fn configure(
        context: u32,
        configuration: JsonValue,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        guard(|| configure(context, configuration, width, height))
    }

    /// `unconfigure()` of an offscreen context.
    pub fn unconfigure(context: u32) {
        unconfigure(context);
    }

    /// `getCurrentTexture()` of an offscreen context: a texture id.
    pub fn current_texture(context: u32) -> Result<u32, String> {
        guard(|| current_texture(context))
    }

    /// `present()` of an offscreen context.
    pub fn present(context: u32) -> Result<(), String> {
        guard(|| present(context))
    }

    /// `[width, height]` of an offscreen context's textures, or `null`.
    pub fn size(context: u32) -> Option<Vec<u32>> {
        size(context).map(|(w, h)| vec![w, h])
    }
}
