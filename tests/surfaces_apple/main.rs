//! The canvas context (area: surfaces-apple), called as TSX calls it, on
//! the machine's GPU: its configuration and frames without a view (the
//! offscreen path a canvas takes until its view attaches), and on macOS
//! with a real `CAMetalLayer` surface attached, as the canvas view does.

use std::sync::OnceLock;

use nsh_webgpu::api::{
    adapter::WgpuAdapter,
    buffer::WgpuBuffer,
    canvas_context::{self, WgpuCanvasContext},
    command_encoder::WgpuCommandEncoder,
    gpu::Wgpu,
    queue::WgpuQueue,
    render_pass_encoder::WgpuRenderPassEncoder,
    texture_view::WgpuTextureView,
};
use serde_json::{Value, json};

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .expect("a runtime")
    })
}

fn device() -> u32 {
    let adapter = runtime()
        .block_on(Wgpu::request_adapter(json!({})))
        .expect("requestAdapter")
        .expect("an adapter on this machine");
    runtime()
        .block_on(WgpuAdapter::request_device(adapter, json!({})))
        .expect("requestDevice")
}

/// `GPUTextureUsage.RENDER_ATTACHMENT | COPY_SRC`.
const USAGE: u32 = 0x10 | 0x01;

fn configuration(device: u32, extra: Value, width: u32, height: u32) -> Value {
    let mut c = json!({
        "device": device,
        "format": Wgpu::preferred_canvas_format(),
        "usage": USAGE,
        "alphaMode": "premultiplied",
        "width": width,
        "height": height,
    });
    if let (Some(c), Some(extra)) = (c.as_object_mut(), extra.as_object()) {
        c.extend(extra.clone());
    }
    c
}

/// Clears `texture` to `color` and reads its pixel (x, y) back as RGBA.
fn clear_and_read(
    device: u32,
    texture: u32,
    width: u32,
    color: [f64; 4],
    x: u32,
    y: u32,
) -> [u8; 4] {
    let view = WgpuTextureView::create(texture, json!(null)).unwrap();
    let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
    let pass = WgpuCommandEncoder::begin_render_pass(
        encoder,
        json!({ "colorAttachments": [{ "view": view, "clearValue": color, "loadOp": "clear", "storeOp": "store" }] }),
    )
    .unwrap();
    WgpuRenderPassEncoder::end(pass).unwrap();
    let bytes_per_row = (width * 4).div_ceil(256) * 256;
    let height = y + 1;
    let readback = WgpuBuffer::create(
        device,
        json!({ "size": bytes_per_row * height, "usage": 0x08 | 0x01 }),
    )
    .unwrap();
    WgpuCommandEncoder::copy_texture_to_buffer(
        encoder,
        json!({ "texture": texture }),
        json!({ "buffer": readback, "bytesPerRow": bytes_per_row }),
        json!([width, height]),
    )
    .unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
    let ticket = WgpuBuffer::map_async(readback, 1, None, None).unwrap();
    runtime().block_on(Wgpu::wait(ticket)).unwrap();
    let bytes = WgpuBuffer::get_mapped_range(readback, None, None).unwrap();
    WgpuBuffer::unmap(readback).unwrap();
    let i = (y * bytes_per_row + x * 4) as usize;
    let p = &bytes[i..i + 4];
    // The preferred format is bgra8unorm on Apple, rgba8unorm elsewhere.
    if Wgpu::preferred_canvas_format().starts_with("bgra") {
        [p[2], p[1], p[0], p[3]]
    } else {
        [p[0], p[1], p[2], p[3]]
    }
}

fn error_name(result: Result<impl std::fmt::Debug, String>) -> String {
    let error = result.expect_err("an error");
    error.split(':').next().unwrap_or_default().to_owned()
}

#[test]
fn context_ids_are_unique_and_destroy_drops_the_context() {
    let a = WgpuCanvasContext::allocate_context_id();
    let b = WgpuCanvasContext::allocate_context_id();
    assert_ne!(a, b);
    WgpuCanvasContext::make(a).unwrap();
    let before = canvas_context::context_count();
    // Unmounted with no view: the context goes with its last context object.
    WgpuCanvasContext::destroy_context(a);
    assert_eq!(canvas_context::context_count(), before);
    WgpuCanvasContext::release(a);
    assert_eq!(canvas_context::context_count(), before - 1);
    // A <Canvas> that unmounts with no context object made goes at once.
    let c = WgpuCanvasContext::allocate_context_id();
    WgpuCanvasContext::make(c).unwrap();
    WgpuCanvasContext::release(c);
    let before = canvas_context::context_count();
    WgpuCanvasContext::destroy_context(c);
    assert_eq!(canvas_context::context_count(), before - 1);
    // A destroyed canvas's state is not made again (getContext after unmount).
    let before = canvas_context::context_count();
    assert_eq!(error_name(WgpuCanvasContext::make(c)), "InvalidStateError");
    assert_eq!(canvas_context::context_count(), before);
    let d = WgpuCanvasContext::allocate_context_id();
    WgpuCanvasContext::destroy_context(d);
    assert_eq!(error_name(WgpuCanvasContext::make(d)), "InvalidStateError");
    assert_eq!(canvas_context::context_count(), before);
    // No view: zeros for getNativeSurface.
    assert_eq!(
        WgpuCanvasContext::native_surface(b),
        json!({ "surface": "0", "width": 0.0, "height": 0.0, "scale": 0.0 })
    );
}

#[test]
fn configure_rejects_what_the_spec_rejects() {
    let device = device();
    let id = WgpuCanvasContext::allocate_context_id();
    WgpuCanvasContext::make(id).unwrap();
    let bad = |extra: Value| WgpuCanvasContext::configure(id, configuration(device, extra, 8, 8));
    let color_space = bad(json!({ "colorSpace": "rec2020" })).unwrap_err();
    assert!(color_space.starts_with("TypeError") && color_space.contains("PredefinedColorSpace"));
    let tone = bad(json!({ "toneMapping": { "mode": "loud" } })).unwrap_err();
    assert!(tone.starts_with("TypeError") && tone.contains("GPUCanvasToneMappingMode"));
    assert_eq!(
        error_name(bad(json!({ "alphaMode": "weird" }))),
        "TypeError"
    );
    assert_eq!(error_name(bad(json!({ "format": "r8unorm" }))), "TypeError");
    assert_eq!(error_name(bad(json!({ "device": 999_999 }))), "TypeError");
    for ok in [
        json!({ "colorSpace": "display-p3" }),
        json!({ "toneMapping": { "mode": "extended" }, "format": "rgba16float" }),
        json!({ "alphaMode": "opaque" }),
    ] {
        bad(ok).unwrap();
    }
    WgpuCanvasContext::destroy_context(id);
}

#[test]
fn frames_offscreen_until_a_view_attaches() {
    let device = device();
    let id = WgpuCanvasContext::allocate_context_id();
    WgpuCanvasContext::make(id).unwrap();
    assert_eq!(
        error_name(WgpuCanvasContext::get_current_texture(id, 16, 16)),
        "InvalidStateError"
    );
    WgpuCanvasContext::configure(id, configuration(device, json!({}), 16, 16)).unwrap();
    assert!(!WgpuCanvasContext::has_surface(id));
    let first = WgpuCanvasContext::get_current_texture(id, 16, 16).unwrap();
    assert_eq!(
        WgpuCanvasContext::get_current_texture(id, 16, 16).unwrap(),
        first,
        "one texture per frame"
    );
    assert_eq!(
        clear_and_read(device, first, 16, [0.0, 1.0, 0.0, 1.0], 3, 3),
        [0, 255, 0, 255]
    );
    WgpuCanvasContext::present(id).unwrap();
    let second = WgpuCanvasContext::get_current_texture(id, 16, 16).unwrap();
    assert_ne!(second, first, "a new texture after present");
    let resized = WgpuCanvasContext::get_current_texture(id, 32, 16).unwrap();
    assert_ne!(resized, second, "a new texture after a resize");
    WgpuCanvasContext::configure(id, configuration(device, json!({}), 32, 16)).unwrap();
    let reconfigured = WgpuCanvasContext::get_current_texture(id, 32, 16).unwrap();
    assert_ne!(reconfigured, resized, "a new texture after configure");
    WgpuCanvasContext::unconfigure(id).unwrap();
    assert_eq!(
        error_name(WgpuCanvasContext::get_current_texture(id, 32, 16)),
        "InvalidStateError"
    );
    WgpuCanvasContext::destroy_context(id);
}

/// A `CAMetalLayer` surface attached as the macOS canvas view attaches its
/// own (`surface/apple.rs`): frames come from the layer's drawables, and a
/// frame presented before the layer attached is shown on it when it does.
#[cfg(target_os = "macos")]
#[test]
fn frames_come_from_an_attached_metal_layer() {
    use nsh_webgpu::surface::views;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_quartz_core::CAMetalLayer;

    let device = device();
    let id = WgpuCanvasContext::allocate_context_id();
    WgpuCanvasContext::make(id).unwrap();
    WgpuCanvasContext::configure(id, configuration(device, json!({}), 64, 64)).unwrap();
    // A frame before the layer: offscreen, presented, kept.
    let offscreen = WgpuCanvasContext::get_current_texture(id, 64, 64).unwrap();
    clear_and_read(device, offscreen, 64, [0.0, 0.0, 1.0, 1.0], 0, 0);
    WgpuCanvasContext::present(id).unwrap();

    let layer = CAMetalLayer::new();
    layer.setBounds(CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(32.0, 32.0)));
    layer.setContentsScale(2.0);
    let pointer = std::ptr::from_ref(&*layer) as *mut std::ffi::c_void;
    // SAFETY: the layer lives until the end of the test, after the surface.
    let surface = unsafe {
        nsh_webgpu::instance::instance()
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(pointer))
    }
    .expect("a surface for the layer");
    views::attach(id, surface, 64, 64);
    canvas_context::set_native(id, pointer as usize, 32.0, 32.0, 2.0);
    // Configures the layer and shows the offscreen frame on it.
    canvas_context::surface_attached(id);
    assert!(WgpuCanvasContext::has_surface(id));
    assert_eq!(
        layer.drawableSize(),
        CGSize::new(64.0, 64.0),
        "configured at the canvas size"
    );
    let native = WgpuCanvasContext::native_surface(id);
    assert_eq!(native["surface"], json!((pointer as usize).to_string()));
    assert_eq!(native["width"], json!(32.0));

    let drawn = WgpuCanvasContext::get_current_texture(id, 64, 64).unwrap();
    assert_ne!(drawn, offscreen);
    assert_eq!(
        clear_and_read(device, drawn, 64, [1.0, 0.0, 0.0, 1.0], 10, 10),
        [255, 0, 0, 255]
    );
    WgpuCanvasContext::present(id).unwrap();
    // A new drawing buffer size reconfigures the layer.
    WgpuCanvasContext::get_current_texture(id, 48, 40).unwrap();
    assert_eq!(layer.drawableSize(), CGSize::new(48.0, 40.0));
    WgpuCanvasContext::present(id).unwrap();

    // The <Canvas> unmounts first, then its view goes, then its context
    // object is collected: the context goes with the last of them.
    let before = canvas_context::context_count();
    WgpuCanvasContext::destroy_context(id);
    assert_eq!(
        canvas_context::context_count(),
        before,
        "kept while the view is there"
    );
    canvas_context::view_gone(id);
    assert!(!WgpuCanvasContext::has_surface(id));
    assert_eq!(
        canvas_context::context_count(),
        before,
        "kept while a context object holds it"
    );
    WgpuCanvasContext::release(id);
    assert_eq!(canvas_context::context_count(), before - 1);
}

/// A `CAMetalLayer` surface as the canvas view attaches it, for `id`.
#[cfg(target_os = "macos")]
fn attach_layer(id: u32) -> objc2::rc::Retained<objc2_quartz_core::CAMetalLayer> {
    use nsh_webgpu::surface::views;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_quartz_core::CAMetalLayer;

    let layer = CAMetalLayer::new();
    layer.setBounds(CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(32.0, 32.0)));
    layer.setContentsScale(2.0);
    let pointer = std::ptr::from_ref(&*layer) as *mut std::ffi::c_void;
    // SAFETY: the layer is returned to the test, which keeps it past the
    // surface (wgpu retains it too).
    let surface = unsafe {
        nsh_webgpu::instance::instance()
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(pointer))
    }
    .expect("a surface for the layer");
    views::attach(id, surface, 64, 64);
    canvas_context::set_native(id, pointer as usize, 32.0, 32.0, 2.0);
    canvas_context::surface_attached(id);
    layer
}

/// A view of a past frame that JS has not collected does not outlive the
/// next configuration: DX12 cannot resize a swap chain while one lives
/// (found on Windows by the gallery's Context Edge Cases). It stays a view,
/// of a destroyed texture.
#[cfg(target_os = "macos")]
#[test]
fn a_past_frames_view_expires_when_the_canvas_is_reconfigured() {
    use nsh_webgpu::api::texture_view::{TextureViewEntry, WgpuTextureView};

    let device = device();
    let id = WgpuCanvasContext::allocate_context_id();
    WgpuCanvasContext::make(id).unwrap();
    WgpuCanvasContext::configure(id, configuration(device, json!({}), 64, 64)).unwrap();
    let _layer = attach_layer(id);
    let size = |view: u32| {
        let entry = TextureViewEntry::lookup(view).expect("the view stays");
        entry.view.texture().size().width
    };
    let frame = WgpuCanvasContext::get_current_texture(id, 64, 64).unwrap();
    let held = WgpuTextureView::create(frame, json!({})).unwrap();
    WgpuCanvasContext::present(id).unwrap();
    // The same size: nothing is reconfigured.
    WgpuCanvasContext::get_current_texture(id, 64, 64).unwrap();
    assert_eq!(size(held), 64);
    WgpuCanvasContext::present(id).unwrap();
    // Another size: it is.
    let resized = WgpuCanvasContext::get_current_texture(id, 48, 40).unwrap();
    assert_eq!(
        size(held),
        1,
        "the past frame's view still holds its buffer"
    );
    let current = WgpuTextureView::create(resized, json!({})).unwrap();
    assert_eq!(size(current), 48);
    WgpuCanvasContext::present(id).unwrap();
    // configure() expires them too.
    WgpuCanvasContext::configure(id, configuration(device, json!({}), 48, 40)).unwrap();
    assert_eq!(size(current), 1);
    WgpuCanvasContext::destroy_context(id);
    canvas_context::view_gone(id);
    WgpuCanvasContext::release(id);
}

/// The crash of docs/evidence/gallery/crash-*.txt: a page left while its
/// frame was acquired from the layer and not presented. The view's teardown
/// (`apple::detach`, from the view's `dealloc` on the main thread) used to
/// drop the surface and then the frame's `SurfaceTexture`, and wgpu-core
/// panicked ("Surface[..] does not exist"), which aborts inside `dealloc`.
/// Now the frame ends first, and the context renders offscreen after.
#[cfg(target_os = "macos")]
#[test]
fn a_view_going_with_an_unpresented_frame_does_not_panic() {
    let device = device();
    for unmount_first in [true, false] {
        let id = WgpuCanvasContext::allocate_context_id();
        WgpuCanvasContext::make(id).unwrap();
        WgpuCanvasContext::configure(id, configuration(device, json!({}), 64, 64)).unwrap();
        let layer = attach_layer(id);
        let acquired = WgpuCanvasContext::get_current_texture(id, 64, 64).unwrap();
        clear_and_read(device, acquired, 64, [1.0, 0.0, 0.0, 1.0], 1, 1);
        // Not presented. The <Canvas> unmounts (before or after its view
        // goes, as the platforms order it), and the view goes.
        if unmount_first {
            WgpuCanvasContext::destroy_context(id);
        }
        let panics = canvas_context::teardown_panics();
        let teardown = std::panic::catch_unwind(|| canvas_context::view_gone(id));
        assert!(teardown.is_ok(), "view_gone let a panic out");
        assert_eq!(
            canvas_context::teardown_panics(),
            panics,
            "the view's teardown panicked (caught)"
        );
        if !unmount_first {
            WgpuCanvasContext::destroy_context(id);
        }
        assert!(!WgpuCanvasContext::has_surface(id));
        // The context keeps its configuration and renders offscreen.
        let offscreen = WgpuCanvasContext::get_current_texture(id, 64, 64).unwrap();
        assert_ne!(offscreen, acquired);
        assert_eq!(
            clear_and_read(device, offscreen, 64, [0.0, 1.0, 0.0, 1.0], 2, 2),
            [0, 255, 0, 255]
        );
        WgpuCanvasContext::present(id).unwrap();
        let before = canvas_context::context_count();
        WgpuCanvasContext::release(id);
        assert_eq!(canvas_context::context_count(), before - 1);
        drop(layer);
    }
}
