//! What the resources tests share: a runtime, devices, error scopes, raw
//! wgpu commands on the module's objects (for the commands another area
//! implements), and the reference's image check.

use std::{path::PathBuf, sync::OnceLock};

use nsh_webgpu::api::{adapter::WgpuAdapter, buffer, device::WgpuDevice, gpu::Wgpu};
use serde_json::{Value, json};

/// The runtime the async functions run on.
pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .expect("a runtime")
    })
}

/// Runs `future` to its end.
pub(crate) fn block<F: Future>(future: F) -> F::Output {
    runtime().block_on(future)
}

/// A new adapter.
pub(crate) fn adapter() -> u32 {
    block(Wgpu::request_adapter(json!({})))
        .expect("requestAdapter")
        .expect("an adapter on this machine")
}

/// A new device with `descriptor`.
pub(crate) fn device_with(descriptor: Value) -> u32 {
    block(WgpuAdapter::request_device(adapter(), descriptor)).expect("requestDevice")
}

/// A new device.
pub(crate) fn device() -> u32 {
    device_with(json!({}))
}

/// Waits for ticket `ticket`.
pub(crate) fn wait(ticket: u32) -> Result<(), String> {
    block(Wgpu::wait(ticket))
}

/// The validation error `f` makes on `device` (its message), or `None`.
pub(crate) fn validation_error<T>(device: u32, f: impl FnOnce() -> T) -> (T, Option<String>) {
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    let value = f();
    let error = WgpuDevice::pop_error_scope(device).unwrap();
    let message = (!error.is_null()).then(|| error["message"].as_str().unwrap().to_owned());
    (value, message)
}

/// Asserts `result` is an exception of class `name`.
#[track_caller]
pub(crate) fn assert_throws<T: std::fmt::Debug>(result: Result<T, String>, name: &str) {
    let error = result.expect_err("an exception");
    assert!(
        error.starts_with(&format!("{name}:")),
        "expected {name}, got {error}"
    );
}

/// The wgpu device and queue behind device `device`.
pub(crate) fn wgpu(device: u32) -> (wgpu::Device, wgpu::Queue) {
    buffer::wgpu_device(device).expect("a device")
}

/// Submits what `record` records on `device`, then waits for the GPU.
pub(crate) fn submit(device: u32, record: impl FnOnce(&mut wgpu::CommandEncoder)) {
    let (d, q) = wgpu(device);
    let mut encoder = d.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    record(&mut encoder);
    q.submit([encoder.finish()]);
    d.poll(wgpu::PollType::wait_indefinitely()).unwrap();
}

/// `examples/headless/snapshots/resources/<name>` (the reference's
/// snapshots, copied).
pub(crate) fn snapshot(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples/headless/snapshots/resources")
        .join(name)
}

/// The reference's `checkImage` (setup.ts): `rgba` against snapshot
/// `name`, same size, at most 200 pixels differing. A pixel differs when a
/// channel is more than 10% of its range off (pixelmatch's `threshold:
/// 0.1`, on channels rather than its YIQ distance). Saves the image under
/// `NSH_WEBGPU_EVIDENCE` when set. The number of differing pixels.
pub(crate) fn check_image(rgba: &image::RgbaImage, name: &str) -> usize {
    if let Ok(dir) = std::env::var("NSH_WEBGPU_EVIDENCE") {
        rgba.save(PathBuf::from(dir).join(format!("{name}.test.png")))
            .unwrap();
    }
    let reference = image::open(snapshot(name)).unwrap().to_rgba8();
    assert_eq!(
        reference.dimensions(),
        rgba.dimensions(),
        "image sizes don't match"
    );
    let differing = reference
        .pixels()
        .zip(rgba.pixels())
        .filter(|(a, b)| a.0.iter().zip(b.0).any(|(a, b)| a.abs_diff(b) > 25))
        .count();
    assert!(differing <= 200, "{differing} pixels differ from {name}");
    differing
}
