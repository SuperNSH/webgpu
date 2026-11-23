//! Audit repros (not part of the crate's tests). Run:
//! CARGO_TARGET_DIR=../../../../target cargo test --manifest-path docs/evidence/audit/repro/Cargo.toml -- --nocapture --test-threads=1
use std::sync::OnceLock;

use nsh_webgpu::api::{
    adapter::WgpuAdapter, buffer::WgpuBuffer, canvas_context::{self, WgpuCanvasContext},
    command_encoder::WgpuCommandEncoder, gpu::Wgpu, queue::WgpuQueue,
    render_pass_encoder::WgpuRenderPassEncoder, texture_view::WgpuTextureView,
};
use serde_json::json;

fn runtime() -> &'static tokio::runtime::Runtime {
    static R: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    R.get_or_init(|| tokio::runtime::Builder::new_multi_thread().worker_threads(1).build().unwrap())
}

fn device() -> u32 {
    let adapter = runtime().block_on(Wgpu::request_adapter(json!({}))).unwrap().unwrap();
    runtime().block_on(WgpuAdapter::request_device(adapter, json!({}))).unwrap()
}

fn attach_layer(id: u32) -> objc2::rc::Retained<objc2_quartz_core::CAMetalLayer> {
    use nsh_webgpu::surface::views;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_quartz_core::CAMetalLayer;
    let layer = CAMetalLayer::new();
    layer.setBounds(CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(32.0, 32.0)));
    layer.setContentsScale(2.0);
    let pointer = std::ptr::from_ref(&*layer) as *mut std::ffi::c_void;
    let surface = unsafe {
        nsh_webgpu::instance::instance()
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(pointer))
    }
    .unwrap();
    views::attach(id, surface, 64, 64);
    canvas_context::set_native(id, pointer as usize, 32.0, 32.0, 2.0);
    canvas_context::surface_attached(id);
    layer
}

/// The JS thread holds the frame's GPUTexture (registry id) while the UI
/// thread tears the view down, then renders to it and lets it go.
#[test]
fn frame_texture_used_and_released_after_view_gone() {
    let device = device();
    let id = WgpuCanvasContext::allocate_context_id();
    WgpuCanvasContext::make(id);
    WgpuCanvasContext::configure(
        id,
        json!({"device": device, "format": "bgra8unorm", "usage": 0x11, "width": 64, "height": 64}),
    )
    .unwrap();
    let layer = attach_layer(id);
    let frame = WgpuCanvasContext::get_current_texture(id, 64, 64).unwrap();
    canvas_context::view_gone(id);
    drop(layer);
    println!("teardown panics caught: {}", canvas_context::teardown_panics());
    // JS keeps going with the texture it got before the view went.
    let view = WgpuTextureView::create(frame, json!(null));
    println!("createView after view gone: {view:?}");
    if let Ok(view) = view {
        let enc = WgpuCommandEncoder::create(device, json!(null)).unwrap();
        let pass = WgpuCommandEncoder::begin_render_pass(
            enc,
            json!({"colorAttachments": [{"view": view, "clearValue": [1,0,0,1], "loadOp": "clear", "storeOp": "store"}]}),
        );
        println!("beginRenderPass: {pass:?}");
        if let Ok(pass) = pass {
            println!("end: {:?}", WgpuRenderPassEncoder::end(pass));
        }
        let cb = WgpuCommandEncoder::finish(enc, json!(null));
        println!("finish: {cb:?}");
        if let Ok(cb) = cb {
            println!("submit: {:?}", WgpuQueue::submit(device, vec![cb]));
        }
        println!("present: {:?}", WgpuCanvasContext::present(id));
        let r = std::panic::catch_unwind(|| Wgpu::release(view));
        println!("release(view) panicked: {}", r.is_err());
    }
    // The FinalizationRegistry callback: Wgpu.release is not guarded.
    let r = std::panic::catch_unwind(|| Wgpu::release(frame));
    println!("release(frame texture) panicked: {}", r.is_err());
    assert!(r.is_ok(), "Wgpu::release panicked: this unwinds into the JS engine's finalizer");
}

/// A ticket whose promise is never awaited stays in the pending table
/// with a clone of its device (leak check of pending.rs).
#[test]
fn buffer_map_async_getmappedrange_overflow_in_rust_api() {
    let device = device();
    let b = WgpuBuffer::create(device, json!({"size": 16, "usage": 0x01 | 0x08})).unwrap();
    // offset + size wraps u64 in release builds (JS caps at 2^53, Rust callers do not).
    let r = std::panic::catch_unwind(|| WgpuBuffer::map_async(b, 1, Some(9.223372036854775808e18), Some(9.223372036854775808e18)));
    println!("map_async(2^63, 2^63): {r:?}");
}
