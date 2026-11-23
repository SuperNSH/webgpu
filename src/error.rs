//! Errors, both kinds WebGPU has.
//!
//! - **Exceptions** ([`NativeError`]): what the spec throws or rejects with
//!   at once (a `TypeError` for a descriptor that does not convert, an
//!   `OperationError` for `getMappedRange` on an unmapped buffer). A
//!   native function returns `Err(String)` as `"<Name>: <message>"`; the
//!   TSX side (`tsx/internal/native.ts`) rethrows it as that class.
//! - **GPU errors** ([`ErrorSink`]): what the device reports for invalid
//!   use (`GPUValidationError`, `GPUOutOfMemoryError`, `GPUInternalError`).
//!   Each device installs wgpu's uncaptured-error handler, which hands every
//!   error to the device's sink. The sink keeps the spec's error-scope stack
//!   itself (per device, not per thread as wgpu's own scopes are), and an
//!   error no scope catches goes to the device's `uncapturederror`
//!   listener.

use std::{
    cell::RefCell,
    collections::BTreeMap,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use nsh_core::prelude::*;

use crate::registry::Id;

/// What the spec throws, by the class it throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorName {
    /// `TypeError`: an argument or descriptor that does not convert.
    TypeError,
    /// `RangeError`.
    RangeError,
    /// `DOMException` `OperationError`.
    OperationError,
    /// `DOMException` `AbortError`.
    AbortError,
    /// `DOMException` `InvalidStateError`.
    InvalidStateError,
    /// Not available on this platform or in wgpu: `ERR_WEBGPU_UNSUPPORTED`.
    Unsupported,
}

impl ErrorName {
    fn as_str(self) -> &'static str {
        match self {
            ErrorName::TypeError => "TypeError",
            ErrorName::RangeError => "RangeError",
            ErrorName::OperationError => "OperationError",
            ErrorName::AbortError => "AbortError",
            ErrorName::InvalidStateError => "InvalidStateError",
            ErrorName::Unsupported => "ERR_WEBGPU_UNSUPPORTED",
        }
    }
}

/// An exception for TSX: its class and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeError {
    /// The class TSX throws.
    pub name: ErrorName,
    /// The message.
    pub message: String,
}

impl NativeError {
    /// An error of class `name`.
    pub fn new(name: ErrorName, message: impl Into<String>) -> Self {
        Self {
            name,
            message: message.into(),
        }
    }

    /// A `TypeError`.
    pub fn type_error(message: impl Into<String>) -> Self {
        Self::new(ErrorName::TypeError, message)
    }

    /// An `OperationError`.
    pub fn operation(message: impl Into<String>) -> Self {
        Self::new(ErrorName::OperationError, message)
    }

    /// `ERR_WEBGPU_UNSUPPORTED` for `what`.
    pub fn unsupported(what: &str) -> Self {
        Self::new(
            ErrorName::Unsupported,
            format!("{what} is not supported by nsh-webgpu (see docs/parity.md)"),
        )
    }
}

impl fmt::Display for NativeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name.as_str(), self.message)
    }
}

impl From<NativeError> for String {
    fn from(e: NativeError) -> Self {
        e.to_string()
    }
}

/// The result of a native call: `Err` crosses as `"<Name>: <message>"`.
pub type NativeResult<T> = Result<T, String>;

/// Runs `f` for a native call: its error as the string TSX rethrows, and a
/// panic (wgpu panics on some misuse it does not validate) as an
/// `OperationError` instead of unwinding into the JS engine.
pub(crate) fn guard<T>(f: impl FnOnce() -> Result<T, NativeError>) -> NativeResult<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result.map_err(String::from),
        Err(panic) => {
            let message = panic
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap_or("a panic in wgpu");
            Err(NativeError::operation(format!("wgpu: {message}")).into())
        }
    }
}

/// A GPU error's kind: the `GPUError` subclass TSX makes of it, and the
/// `GPUErrorFilter` that catches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuErrorKind {
    /// `GPUValidationError`, filter `"validation"`.
    Validation,
    /// `GPUOutOfMemoryError`, filter `"out-of-memory"`.
    OutOfMemory,
    /// `GPUInternalError`, filter `"internal"`.
    Internal,
}

impl GpuErrorKind {
    /// The filter name.
    pub fn as_str(self) -> &'static str {
        match self {
            GpuErrorKind::Validation => "validation",
            GpuErrorKind::OutOfMemory => "out-of-memory",
            GpuErrorKind::Internal => "internal",
        }
    }

    /// The kind filter `name` names.
    pub fn parse(name: &str) -> Result<Self, NativeError> {
        match name {
            "validation" => Ok(GpuErrorKind::Validation),
            "out-of-memory" => Ok(GpuErrorKind::OutOfMemory),
            "internal" => Ok(GpuErrorKind::Internal),
            other => Err(NativeError::type_error(format!(
                "'{other}' is not a valid GPUErrorFilter"
            ))),
        }
    }
}

/// A GPU error: its kind and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuError {
    /// The kind.
    pub kind: GpuErrorKind,
    /// The message.
    pub message: String,
}

impl From<wgpu::Error> for GpuError {
    fn from(e: wgpu::Error) -> Self {
        let (kind, message) = match e {
            wgpu::Error::OutOfMemory { source } => (GpuErrorKind::OutOfMemory, source.to_string()),
            wgpu::Error::Validation { description, .. } => (GpuErrorKind::Validation, description),
            wgpu::Error::Internal { description, .. } => (GpuErrorKind::Internal, description),
        };
        GpuError { kind, message }
    }
}

/// One open error scope.
#[derive(Debug)]
struct Scope {
    filter: GpuErrorKind,
    /// The first error it caught (the spec keeps the first only).
    error: Option<GpuError>,
}

/// A device's GPU errors: its error scopes, and the uncaptured errors that
/// arrived off the JS thread and wait for TSX to collect them.
#[derive(Debug)]
pub(crate) struct ErrorSink {
    device: Id,
    scopes: Mutex<Vec<Scope>>,
    pending: Mutex<Vec<GpuError>>,
    /// Set when the device was destroyed: an error no scope catches is
    /// dropped (a lost device reports none), instead of queued for a
    /// listener that is gone.
    silenced: AtomicBool,
}

thread_local! {
    /// The `uncapturederror` listeners, by device: TSX functions, which
    /// live on the JS thread only.
    // Already `const`: clippy's missing_const_for_thread_local misreads the
    // expansion on targets without native TLS (Android's emulated TLS).
    #[allow(clippy::missing_const_for_thread_local)]
    static UNCAPTURED: RefCell<BTreeMap<Id, Callback<(String, String)>>> =
        const { RefCell::new(BTreeMap::new()) };
}

impl ErrorSink {
    /// The sink of device `device`.
    pub(crate) fn new(device: Id) -> Arc<Self> {
        Arc::new(Self {
            device,
            scopes: Mutex::new(Vec::new()),
            pending: Mutex::new(Vec::new()),
            silenced: AtomicBool::new(false),
        })
    }

    /// Reports `error`: to the innermost scope whose filter matches, else
    /// to the `uncapturederror` listener (or the pending queue, off the JS
    /// thread). Called by wgpu's handler on the thread that made the error.
    pub(crate) fn report(&self, error: GpuError) {
        {
            let mut scopes = self.scopes.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(scope) = scopes.iter_mut().rev().find(|s| s.filter == error.kind) {
                scope.error.get_or_insert(error);
                return;
            }
        }
        if self.silenced.load(Ordering::Relaxed) {
            return;
        }
        let delivered = UNCAPTURED
            .try_with(|listeners| {
                listeners.borrow().get(&self.device).map(|listener| {
                    listener.post((error.kind.as_str().to_owned(), error.message.clone()));
                })
            })
            .ok()
            .flatten()
            .is_some();
        if !delivered {
            self.pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(error);
        }
    }

    /// `pushErrorScope(filter)`.
    pub(crate) fn push(&self, filter: GpuErrorKind) {
        self.scopes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Scope {
                filter,
                error: None,
            });
    }

    /// `popErrorScope()`: the error the innermost scope caught; an
    /// `OperationError` when no scope is open.
    pub(crate) fn pop(&self) -> Result<Option<GpuError>, NativeError> {
        self.scopes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop()
            .map(|s| s.error)
            .ok_or_else(|| NativeError::operation("popErrorScope: the error scope stack is empty"))
    }

    /// The uncaptured errors that arrived off the JS thread, in order.
    pub(crate) fn take_pending(&self) -> Vec<GpuError> {
        std::mem::take(&mut *self.pending.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Sets the device's `uncapturederror` listener (JS thread).
    pub(crate) fn listen(device: Id, listener: Callback<(String, String)>) {
        UNCAPTURED.with(|l| l.borrow_mut().insert(device, listener));
    }

    /// Drops the device's listener (the device was released).
    pub(crate) fn forget(device: Id) {
        // Taken out before it drops: dropping a TSX function outside the
        // borrow keeps a re-entrant drop from finding the map borrowed.
        let listener = UNCAPTURED
            .try_with(|l| l.borrow_mut().remove(&device))
            .ok()
            .flatten();
        drop(listener);
    }

    /// The device was destroyed: errors no scope catches are dropped from
    /// now on, and the queued ones go.
    pub(crate) fn silence(&self) {
        self.silenced.store(true, Ordering::Relaxed);
        drop(self.take_pending());
    }

    /// The handler wgpu calls with every error of the device.
    pub(crate) fn install(self: &Arc<Self>, device: &wgpu::Device) {
        let sink = Arc::clone(self);
        device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| sink.report(e.into())));
    }
}

/// A pipeline `create*PipelineAsync` could not make: the
/// `GPUPipelineError` TSX rejects with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PipelineError {
    /// `"validation"` or `"internal"` (`GPUPipelineErrorReason`).
    pub(crate) reason: &'static str,
    /// The message.
    pub(crate) message: String,
}

impl PipelineError {
    /// `{ error: { reason, message } }`, as TSX reads it.
    pub(crate) fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "error": { "reason": self.reason, "message": self.message } })
    }
}

/// Runs `f` (which makes a pipeline) on this thread inside wgpu's own
/// validation and internal error scopes, which are per thread: a blocking
/// thread's scopes catch its pipeline's errors, and neither the device's
/// scopes (`ErrorSink`) nor `uncapturederror` see them. A panic in wgpu is
/// an internal error.
pub(crate) fn in_pipeline_scopes<T>(
    device: &wgpu::Device,
    f: impl FnOnce() -> T,
) -> Result<T, PipelineError> {
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    let made = catch_unwind(AssertUnwindSafe(f));
    // Innermost first.
    let internal = pollster::block_on(internal.pop());
    let validation = pollster::block_on(validation.pop());
    let made = made.map_err(|panic| PipelineError {
        reason: "internal",
        message: panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_else(|| "a panic in wgpu".to_owned()),
    })?;
    if let Some(error) = validation {
        return Err(PipelineError {
            reason: "validation",
            message: GpuError::from(error).message,
        });
    }
    if let Some(error) = internal {
        return Err(PipelineError {
            reason: "internal",
            message: GpuError::from(error).message,
        });
    }
    Ok(made)
}
