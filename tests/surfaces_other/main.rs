//! Area surfaces-other: the offscreen canvas (`surface/offscreen.rs`), on
//! this machine's GPU, called as TSX and the platform views call it. The
//! platform views themselves (Windows, Linux, Android) are checked by
//! compiling for their targets and by the example apps
//! (docs/parity/surfaces-other.md).

use std::{
    sync::{Arc, Mutex, OnceLock, mpsc},
    time::Duration,
};

use nsh_webgpu::{
    api::{
        adapter::WgpuAdapter, canvas_context::WgpuCanvasContext,
        command_encoder::WgpuCommandEncoder, gpu::Wgpu, queue::WgpuQueue,
        render_pass_encoder::WgpuRenderPassEncoder, render_pipeline::WgpuRenderPipeline,
        shader_module::WgpuShaderModule, texture::WgpuTexture, texture_view::WgpuTextureView,
    },
    surface::offscreen::{self, Frame, WgpuOffscreenCanvas},
};
use serde_json::json;

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .expect("a runtime")
    })
}

fn block<F: Future>(future: F) -> F::Output {
    runtime().block_on(future)
}

fn device() -> u32 {
    let adapter = block(Wgpu::request_adapter(json!({})))
        .expect("requestAdapter")
        .expect("an adapter on this machine");
    block(WgpuAdapter::request_device(adapter, json!({}))).expect("requestDevice")
}

/// A full-screen triangle whose fragments are `(x, y, 0.5, 1)` with x, y
/// the pixel's column and row over 255: each pixel says where it is, so a
/// readback that loses padding or flips rows shows.
const GRADIENT_WGSL: &str = "
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  var pos = array<vec2f, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
  return vec4f(pos[i], 0.0, 1.0);
}
@fragment
fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
  return vec4f(floor(p.x) / 255.0, floor(p.y) / 255.0, 0.5, 1.0);
}";

/// `GPUTextureUsage.COPY_SRC | RENDER_ATTACHMENT`.
const USAGE: u32 = 0x01 | 0x10;

/// Draws the gradient into texture `texture` (of `format`) on `device`.
fn draw_gradient(device: u32, texture: u32, format: &str) {
    let module = WgpuShaderModule::create(device, json!({ "code": GRADIENT_WGSL })).unwrap();
    let pipeline = WgpuRenderPipeline::create(
        device,
        json!({
            "layout": "auto",
            "vertex": { "module": module, "entryPoint": "vs" },
            "fragment": { "module": module, "entryPoint": "fs", "targets": [{ "format": format }] },
            "primitive": { "topology": "triangle-list" },
        }),
    )
    .unwrap();
    let view = WgpuTextureView::create(texture, json!(null)).unwrap();
    let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
    let pass = WgpuCommandEncoder::begin_render_pass(
        encoder,
        json!({
            "colorAttachments": [{
                "view": view, "clearValue": [0, 0, 0, 0], "loadOp": "clear", "storeOp": "store",
            }],
        }),
    )
    .unwrap();
    WgpuRenderPassEncoder::set_pipeline(pass, pipeline).unwrap();
    WgpuRenderPassEncoder::draw(pass, 3, None, None, None).unwrap();
    WgpuRenderPassEncoder::end(pass).unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
}

/// The gradient's RGBA bytes for a `width` × `height` image.
fn expected_gradient(width: u32, height: u32) -> Vec<u8> {
    (0..height)
        .flat_map(|y| (0..width).flat_map(move |x| [x as u8, y as u8, 128, 255]))
        .collect()
}

/// Whether `actual` is `expected` within 1 per channel (0.5 rounds either
/// way on some GPUs).
fn close(actual: &[u8], expected: &[u8]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(a, e)| a.abs_diff(*e) <= 1)
}

#[test]
fn reads_back_a_texture_whose_rows_need_padding() {
    // 13 × 7: 52-byte rows, padded to 256 for the copy. The reference's
    // getImageData copies with bytesPerRow = width * 4 and fails here.
    let device = device();
    let texture = WgpuTexture::create(
        device,
        json!({ "size": [13, 7], "format": "rgba8unorm", "usage": USAGE }),
    )
    .unwrap();
    draw_gradient(device, texture, "rgba8unorm");
    let ticket = WgpuOffscreenCanvas::read_pixels(texture).unwrap();
    block(Wgpu::wait(ticket)).unwrap();
    let pixels = WgpuOffscreenCanvas::take_pixels(ticket).unwrap();
    assert_eq!(pixels.len(), 13 * 7 * 4);
    assert!(close(&pixels, &expected_gradient(13, 7)), "{pixels:?}");
    // A ticket is taken once.
    assert!(
        WgpuOffscreenCanvas::take_pixels(ticket)
            .unwrap_err()
            .starts_with("TypeError")
    );
}

#[test]
fn reads_back_bgra_in_its_own_byte_order_and_swizzles_on_request() {
    let device = device();
    let texture = WgpuTexture::create(
        device,
        json!({ "size": [5, 3], "format": "bgra8unorm", "usage": USAGE }),
    )
    .unwrap();
    draw_gradient(device, texture, "bgra8unorm");
    let frame = offscreen::read_texture(texture).unwrap();
    assert_eq!((frame.width, frame.height), (5, 3));
    assert_eq!(frame.format, wgpu::TextureFormat::Bgra8Unorm);
    // BGRA bytes: blue first.
    assert!(
        close(&frame.data[..4], &[128, 0, 0, 255]),
        "{:?}",
        frame.data
    );
    assert!(close(&frame.to_rgba8().unwrap(), &expected_gradient(5, 3)));
}

#[test]
fn refuses_to_read_a_texture_without_copy_src() {
    let device = device();
    let texture = WgpuTexture::create(
        device,
        json!({ "size": [4, 4], "format": "rgba8unorm", "usage": 0x10 }),
    )
    .unwrap();
    let err = WgpuOffscreenCanvas::read_pixels(texture).unwrap_err();
    assert!(
        err.starts_with("OperationError") && err.contains("COPY_SRC"),
        "{err}"
    );
}

#[test]
fn an_offscreen_context_keeps_its_texture_for_a_frame() {
    let device = device();
    let context = 9_001;
    // Not configured yet: InvalidStateError, as getCurrentTexture is.
    let err = WgpuOffscreenCanvas::current_texture(context).unwrap_err();
    assert!(err.starts_with("InvalidStateError"), "{err}");
    WgpuOffscreenCanvas::configure(
        context,
        json!({ "device": device, "format": "rgba8unorm" }),
        16,
        8,
    )
    .unwrap();
    assert_eq!(WgpuOffscreenCanvas::size(context), Some(vec![16, 8]));
    let first = WgpuOffscreenCanvas::current_texture(context).unwrap();
    assert_eq!(
        WgpuOffscreenCanvas::current_texture(context).unwrap(),
        first
    );
    // `present` ends the frame: a new texture after it.
    WgpuOffscreenCanvas::present(context).unwrap();
    let second = WgpuOffscreenCanvas::current_texture(context).unwrap();
    assert_ne!(second, first);
    // Its texture can be read back (COPY_SRC is added for that).
    draw_gradient(device, second, "rgba8unorm");
    let frame = offscreen::read_texture(second).unwrap();
    assert!(close(&frame.data, &expected_gradient(16, 8)));
    WgpuOffscreenCanvas::unconfigure(context);
    assert!(WgpuOffscreenCanvas::current_texture(context).is_err());
    assert_eq!(WgpuOffscreenCanvas::size(context), None);
}

#[test]
fn a_presenting_view_gets_each_presented_frame_at_its_size() {
    let device = device();
    let context = 9_002;
    let (tx, rx) = mpsc::channel::<Frame>();
    let tx = Arc::new(Mutex::new(tx));
    offscreen::show(
        context,
        Arc::new(move |frame| {
            let _ = tx.lock().unwrap().send(frame);
        }),
        6,
        4,
    );
    // The view's size wins over the size configure() is given.
    WgpuOffscreenCanvas::configure(
        context,
        json!({ "device": device, "format": "bgra8unorm" }),
        100,
        100,
    )
    .unwrap();
    let texture = WgpuOffscreenCanvas::current_texture(context).unwrap();
    draw_gradient(device, texture, "bgra8unorm");
    WgpuOffscreenCanvas::present(context).unwrap();
    let frame = rx.recv_timeout(Duration::from_secs(10)).expect("a frame");
    assert_eq!((frame.width, frame.height), (6, 4));
    assert!(close(&frame.to_rgba8().unwrap(), &expected_gradient(6, 4)));

    // The view grows: the next frame's texture is its new size.
    offscreen::resize(context, 9, 3);
    let texture = WgpuOffscreenCanvas::current_texture(context).unwrap();
    draw_gradient(device, texture, "bgra8unorm");
    WgpuOffscreenCanvas::present(context).unwrap();
    let frame = rx.recv_timeout(Duration::from_secs(10)).expect("a frame");
    assert_eq!((frame.width, frame.height), (9, 3));
    assert!(close(&frame.to_rgba8().unwrap(), &expected_gradient(9, 3)));

    // A present with nothing drawn sends nothing; a hidden view gets none.
    WgpuOffscreenCanvas::present(context).unwrap();
    offscreen::hide(context);
    let texture = WgpuOffscreenCanvas::current_texture(context).unwrap();
    draw_gradient(device, texture, "bgra8unorm");
    WgpuOffscreenCanvas::present(context).unwrap();
    assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    WgpuOffscreenCanvas::unconfigure(context);
}

/// Linux's path through the canvas context: a `<Canvas>` context with no
/// surface in `views` (Linux never attaches one) renders offscreen, and its
/// `present()` reaches the presenting view `linux.rs` shows under its id.
#[test]
fn a_canvas_context_without_a_surface_presents_to_its_presenting_view() {
    let device = device();
    let context = WgpuCanvasContext::allocate_context_id();
    let (tx, rx) = mpsc::channel::<Frame>();
    let tx = Arc::new(Mutex::new(tx));
    offscreen::show(
        context,
        Arc::new(move |frame| {
            let _ = tx.lock().unwrap().send(frame);
        }),
        7,
        5,
    );
    WgpuCanvasContext::make(context).unwrap();
    assert!(!WgpuCanvasContext::has_surface(context));
    WgpuCanvasContext::configure(
        context,
        json!({ "device": device, "format": "bgra8unorm", "alphaMode": "premultiplied",
            "width": 7, "height": 5 }),
    )
    .unwrap();
    let texture = WgpuCanvasContext::get_current_texture(context, 7, 5).unwrap();
    draw_gradient(device, texture, "bgra8unorm");
    WgpuCanvasContext::present(context).unwrap();
    let frame = rx.recv_timeout(Duration::from_secs(10)).expect("a frame");
    assert_eq!((frame.width, frame.height), (7, 5));
    assert!(!frame.opaque);
    assert!(close(&frame.to_rgba8().unwrap(), &expected_gradient(7, 5)));

    // The next frame is drawn and shown too; an opaque context says so.
    WgpuCanvasContext::configure(
        context,
        json!({ "device": device, "format": "bgra8unorm", "width": 7, "height": 5 }),
    )
    .unwrap();
    let texture = WgpuCanvasContext::get_current_texture(context, 7, 5).unwrap();
    draw_gradient(device, texture, "bgra8unorm");
    WgpuCanvasContext::present(context).unwrap();
    let frame = rx.recv_timeout(Duration::from_secs(10)).expect("a frame");
    assert!(frame.opaque);

    // Without a presenting view, a present shows nothing anywhere.
    offscreen::hide(context);
    let texture = WgpuCanvasContext::get_current_texture(context, 7, 5).unwrap();
    draw_gradient(device, texture, "bgra8unorm");
    WgpuCanvasContext::present(context).unwrap();
    assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    WgpuCanvasContext::destroy_context(context);
}
