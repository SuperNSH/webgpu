//! `GPUDevice`: what it is (features, limits), its errors (scopes,
//! `uncapturederror`), `lost` and `destroy()`. Its `create*` functions live
//! with the kinds they make (`WgpuBuffer.create` in `buffer.rs`).

use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use nsh_core::{JsonValue, prelude::*};
use serde_json::json;

use crate::{
    api::{features, limits},
    error::{ErrorSink, GpuError, GpuErrorKind, guard},
    registry::{self, Id},
};

/// Why and how a device was lost: `(reason, message)`, the reason
/// `"destroyed"` or `"unknown"`.
pub(crate) type LostInfo = (String, String);

/// A device in the registry, with its queue (the spec's `device.queue`).
#[derive(Clone)]
pub struct DeviceEntry {
    /// Its own id (errors name it).
    pub id: Id,
    /// The device.
    pub device: wgpu::Device,
    /// Its queue.
    pub queue: wgpu::Queue,
    /// Its errors: scopes and uncaptured ones.
    pub(crate) errors: Arc<ErrorSink>,
    /// Set once the device is lost.
    pub(crate) lost: Arc<Mutex<Option<LostInfo>>>,
    /// The adapter it came from (`requestDevice`), for what only an adapter
    /// tells: a surface's capabilities.
    pub(crate) adapter: Option<wgpu::Adapter>,
}

/// Destroyed devices whose `GPUDevice` object is still alive in TSX: out
/// of the registry (`destroy()` releases the device's id and its TSX
/// listeners), but still found by id, so the objects it made keep behaving
/// as the spec's objects of a lost device do. Each goes when TSX collects
/// its `GPUDevice` ([`WgpuDevice::release`]). The wgpu device inside is
/// destroyed already: its GPU memory is gone, this is the handle.
static RETIRED: Mutex<BTreeMap<Id, DeviceEntry>> = Mutex::new(BTreeMap::new());

fn retired() -> std::sync::MutexGuard<'static, BTreeMap<Id, DeviceEntry>> {
    RETIRED.lock().unwrap_or_else(|e| e.into_inner())
}

thread_local! {
    /// The `lost` listeners, by device (TSX functions: JS thread only).
    // Already `const`: clippy's missing_const_for_thread_local misreads the
    // expansion on targets without native TLS (Android's emulated TLS).
    #[allow(clippy::missing_const_for_thread_local)]
    static LOST: RefCell<BTreeMap<Id, Callback<(String, String)>>> =
        const { RefCell::new(BTreeMap::new()) };
}

impl DeviceEntry {
    /// Keeps `device` and `queue` under a new id, with wgpu's error and
    /// lost callbacks routed to the entry; the id.
    /// `adapter`: the one it came from, when known.
    pub(crate) fn register(
        device: wgpu::Device,
        queue: wgpu::Queue,
        adapter: Option<wgpu::Adapter>,
    ) -> Id {
        let id = registry::next_id();
        let errors = ErrorSink::new(id);
        errors.install(&device);
        let lost: Arc<Mutex<Option<LostInfo>>> = Arc::default();
        let lost_slot = Arc::clone(&lost);
        device.set_device_lost_callback(move |reason, message| {
            let reason = match reason {
                wgpu::DeviceLostReason::Destroyed => "destroyed",
                _ => "unknown",
            };
            let info = (reason.to_owned(), message);
            lost_slot
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_or_insert(info.clone());
            // On the JS thread (destroy() calls back at once): tell TSX now;
            // elsewhere TSX reads it with `lostInfo` after its next wait.
            let _ = LOST.try_with(|l| {
                if let Some(listener) = l.borrow_mut().remove(&id) {
                    listener.post(info);
                }
            });
        });
        registry::insert_at(
            id,
            DeviceEntry {
                id,
                device,
                queue,
                errors,
                lost,
                adapter,
            },
        );
        id
    }

    /// Device `id`'s entry.
    pub fn get(id: Id) -> Result<DeviceEntry, crate::error::NativeError> {
        registry::get::<DeviceEntry>(id).or_else(|e| retired().get(&id).cloned().ok_or(e))
    }

    /// Reports a GPU error the binding finds itself (a command buffer
    /// submitted twice): as wgpu's are, to the scopes or `uncapturederror`.
    pub(crate) fn report(&self, kind: GpuErrorKind, message: impl Into<String>) {
        self.errors.report(GpuError {
            kind,
            message: message.into(),
        });
    }
}

/// The native half of `GPUDevice` (and of `device.queue`'s identity).
#[module]
pub struct WgpuDevice;

#[module]
impl WgpuDevice {
    /// `device.features`: WebGPU feature names.
    pub fn features(device: u32) -> Result<Vec<String>, String> {
        guard(|| Ok(features::names(DeviceEntry::get(device)?.device.features())))
    }

    /// `device.limits`: the spec's limits, by name.
    pub fn limits(device: u32) -> Result<JsonValue, String> {
        guard(|| {
            Ok(json!(limits::to_map(
                &DeviceEntry::get(device)?.device.limits()
            )))
        })
    }

    /// `device.pushErrorScope(filter)`; a `TypeError` for a filter that is
    /// not `"validation"`, `"out-of-memory"` or `"internal"`.
    pub fn push_error_scope(device: u32, filter: String) -> Result<(), String> {
        guard(|| {
            let kind = GpuErrorKind::parse(&filter)?;
            DeviceEntry::get(device)?.errors.push(kind);
            Ok(())
        })
    }

    /// `device.popErrorScope()`: `{ kind, message }` of the error the scope
    /// caught, or `null`; an `OperationError` when no scope is open.
    pub fn pop_error_scope(device: u32) -> Result<JsonValue, String> {
        guard(|| {
            Ok(match DeviceEntry::get(device)?.errors.pop()? {
                Some(e) => json!({ "kind": e.kind.as_str(), "message": e.message }),
                None => JsonValue::Null,
            })
        })
    }

    /// Calls `listener` with `(kind, message)` for each error no scope
    /// catches: the device's `uncapturederror` events.
    pub fn on_uncaptured_error(
        device: u32,
        #[args(kind, message)] listener: Callback<(String, String)>,
    ) {
        ErrorSink::listen(device, listener);
    }

    /// The uncaptured errors that arrived off the JS thread since the last
    /// call (during a wait), as `{ kind, message }`.
    pub fn take_uncaptured_errors(device: u32) -> Result<Vec<JsonValue>, String> {
        guard(|| {
            Ok(DeviceEntry::get(device)?
                .errors
                .take_pending()
                .into_iter()
                .map(|e| json!({ "kind": e.kind.as_str(), "message": e.message }))
                .collect())
        })
    }

    /// Calls `listener` once with `(reason, message)` when the device is
    /// lost on the JS thread (`destroy()`); see `lostInfo` for a loss
    /// elsewhere.
    pub fn on_lost(device: u32, #[args(reason, message)] listener: Callback<(String, String)>) {
        LOST.with(|l| l.borrow_mut().insert(device, listener));
    }

    /// `{ reason, message }` once the device is lost, else `null`.
    pub fn lost_info(device: u32) -> Result<JsonValue, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let lost = entry.lost.lock().unwrap_or_else(|e| e.into_inner()).clone();
            Ok(match lost {
                Some((reason, message)) => json!({ "reason": reason, "message": message }),
                None => JsonValue::Null,
            })
        })
    }

    /// `device.destroy()`: the device is lost with reason `"destroyed"`,
    /// its resources with it; then its id and its TSX listeners are
    /// released (the device's objects still find it, see [`RETIRED`]).
    pub fn destroy(device: u32) -> Result<(), String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            entry.device.destroy();
            entry
                .lost
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_or_insert_with(|| ("destroyed".into(), String::new()));
            entry.errors.silence();
            forget_listeners(device);
            if let Some(entry) = registry::take::<DeviceEntry>(device) {
                retired().insert(device, entry);
            }
            Ok(())
        })
    }

    /// Lets device `device` go (TSX collected its `GPUDevice`): drops its
    /// listeners and its entry, destroyed or not.
    pub fn release(device: u32) -> Result<(), String> {
        forget_listeners(device);
        // A wgpu device's drop can panic; it must not unwind into QuickJS.
        guard(|| {
            let entry = retired().remove(&device);
            // Dropped outside the lock: a wgpu drop may report an error.
            drop(entry);
            registry::release(device);
            Ok(())
        })
    }

    /// How many devices the binding holds, alive or destroyed and not yet
    /// collected (tests of `destroy()` and of the collector).
    pub fn live_count() -> u32 {
        let destroyed = retired().len();
        (registry::count::<DeviceEntry>() + destroyed) as u32
    }
}

/// Drops device `device`'s TSX listeners (`uncapturederror`, `lost`), which
/// hold TSX functions.
fn forget_listeners(device: Id) {
    ErrorSink::forget(device);
    let listener = LOST
        .try_with(|l| l.borrow_mut().remove(&device))
        .ok()
        .flatten();
    drop(listener);
}
