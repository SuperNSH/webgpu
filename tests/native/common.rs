//! What the tests share: a runtime for the async functions, a device.

use std::sync::OnceLock;

use nsh_webgpu::api::{adapter::WgpuAdapter, gpu::Wgpu};
use serde_json::json;

/// The runtime `Wgpu.requestAdapter` and `Wgpu.wait` run on (their
/// blocking work goes to its blocking threads, as in an app).
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

/// A new device: `navigator.gpu.requestAdapter()` then
/// `adapter.requestDevice()`.
pub(crate) fn device() -> u32 {
    let adapter = block(Wgpu::request_adapter(json!({})))
        .expect("requestAdapter")
        .expect("an adapter on this machine");
    block(WgpuAdapter::request_device(adapter, json!({}))).expect("requestDevice")
}

/// Waits for ticket `ticket` (`mapAsync`, `onSubmittedWorkDone`).
pub(crate) fn wait(ticket: u32) -> Result<(), String> {
    block(Wgpu::wait(ticket))
}
