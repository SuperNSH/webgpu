//! `GPU` (`navigator.gpu`) and what every object shares: releasing an id
//! and awaiting a ticket.

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::adapter::AdapterEntry,
    convert::parse,
    error::{NativeError, guard},
    instance::instance,
    pending,
    registry::{self, Id},
};

/// wasm has no `nsh_core::blocking` (one thread, no blocking pool): the
/// work runs in place. Same shape as `blocking`, so the callers do not
/// change; `adapter.rs` and `image_bitmap.rs` import it on wasm.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn blocking<R>(f: impl FnOnce() -> R) -> Result<R, std::convert::Infallible> {
    Ok(f())
}

/// `GPURequestAdapterOptions`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestAdapterOptions {
    #[serde(default)]
    power_preference: Option<wgpu::PowerPreference>,
    #[serde(default)]
    force_fallback_adapter: bool,
    /// `"core"` or `"compatibility"`; wgpu has one level, which serves both.
    #[serde(default)]
    #[allow(dead_code)]
    feature_level: Option<String>,
}

/// The native half of `navigator.gpu` and of every WebGPU object: ids,
/// adapters, pending work.
#[module]
pub struct Wgpu;

#[module]
impl Wgpu {
    /// `navigator.gpu.requestAdapter(options)`: the adapter's id, or `null`
    /// when none fits.
    pub async fn request_adapter(options: JsonValue) -> Result<Option<u32>, String> {
        let options: RequestAdapterOptions =
            parse("GPURequestAdapterOptions", options).map_err(String::from)?;
        let wgpu_options = wgpu::RequestAdapterOptions {
            power_preference: options.power_preference.unwrap_or_default(),
            force_fallback_adapter: options.force_fallback_adapter,
            compatible_surface: None,
        };
        let adapter =
            blocking(move || pollster::block_on(instance().request_adapter(&wgpu_options)))
                .await
                .map_err(|e| NativeError::operation(format!("requestAdapter: {e}")))?;
        Ok(adapter.ok().map(|adapter| {
            registry::insert(AdapterEntry {
                adapter,
                fallback: options.force_fallback_adapter,
            })
        }))
    }

    /// `navigator.gpu.getPreferredCanvasFormat()`: `"rgba8unorm"` on
    /// Android, `"bgra8unorm"` elsewhere, as react-native-webgpu says.
    pub fn preferred_canvas_format() -> String {
        if cfg!(target_os = "android") {
            "rgba8unorm".into()
        } else {
            "bgra8unorm".into()
        }
    }

    /// `navigator.gpu.wgslLanguageFeatures`: the WGSL language extensions
    /// naga accepts, by their WGSL names.
    pub fn wgsl_language_features() -> Vec<String> {
        instance()
            .wgsl_language_features()
            .iter_names()
            .map(|(name, _)| name.to_ascii_lowercase())
            .collect()
    }

    /// Settles when the work of ticket `ticket` (`mapAsync`,
    /// `onSubmittedWorkDone`) is done: polls its device off the JS thread
    /// until wgpu has called back.
    pub async fn wait(ticket: u32) -> Result<(), String> {
        // One wait covers the work submitted before the ticket; a few more
        // cover a submission racing it. wgpu calls back inside the poll.
        for _ in 0..8 {
            let device = match pending::check(ticket)? {
                Ok(outcome) => return outcome.map_err(String::from),
                Err(device) => device,
            };
            let polled = blocking(move || device.poll(wgpu::PollType::wait_indefinitely()))
                .await
                .map_err(|e| e.to_string())
                .and_then(|polled| polled.map(drop).map_err(|e| e.to_string()));
            if let Err(e) = polled {
                // Nobody waits for it any more: the ticket goes.
                pending::forget(ticket);
                return Err(NativeError::operation(format!("poll: {e}")).into());
            }
        }
        pending::forget(ticket);
        Err(NativeError::operation("the GPU did not finish the work").into())
    }

    /// Lets object `id` go: TSX no longer holds it. The wgpu object lives on
    /// while other objects or submitted work use it.
    pub fn release(id: u32) -> Result<(), String> {
        // A wgpu object's drop can panic; it must not unwind into QuickJS.
        guard(|| {
            registry::release(id as Id);
            Ok(())
        })
    }

    /// How many objects the registry holds (for tests of `release`).
    pub fn object_count() -> u32 {
        registry::len() as u32
    }

    /// Whether id `id` is in the registry.
    pub fn alive(id: u32) -> bool {
        registry::contains(id as Id)
    }
}
