//! `GPUBuffer`: `device.createBuffer`, mapping, `destroy()`.
//!
//! Mapping is copy-in/copy-out: `getMappedRange` returns a copy of the
//! mapped bytes (a fresh `ArrayBuffer` in TSX), and `unmap()` writes the
//! ranges TSX got back into the buffer first when it was mapped for writing
//! (`tsx/api/GPUBuffer.ts`). wgpu's mapped memory never leaves Rust, so no
//! JS object points into memory wgpu may unmap.

use std::sync::{Arc, Mutex};

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::device::DeviceEntry,
    bytes::Bytes,
    convert::parse,
    error::{GpuErrorKind, NativeError, guard},
    pending::{self, Slot},
    registry::{self, Id},
};

/// The usage bits WebGPU defines (`GPUBufferUsage`); wgpu's share them.
const SPEC_USAGE: u32 = 0x3ff;
/// `GPUMapMode.READ` / `WRITE`.
const MAP_READ: u32 = 1;
const MAP_WRITE: u32 = 2;

/// Where a buffer's mapping is.
#[derive(Debug, Clone)]
pub(crate) enum MapState {
    /// Not mapped.
    Unmapped,
    /// `mapAsync` is waiting for the GPU; its ticket's slot.
    Pending(Slot),
    /// Mapped: `[offset, offset + size)`, for writing or not.
    Mapped { offset: u64, size: u64, write: bool },
}

/// A buffer in the registry.
#[derive(Clone)]
pub struct BufferEntry {
    /// The buffer.
    pub buffer: wgpu::Buffer,
    /// Its device.
    pub device: Id,
    /// Its mapping.
    pub(crate) map: Arc<Mutex<MapState>>,
}

/// The wgpu device and queue of device `device`, for Rust callers that
/// record wgpu commands of their own on the objects of this module (the
/// tests do). `None`: no such device.
pub fn wgpu_device(device: u32) -> Option<(wgpu::Device, wgpu::Queue)> {
    DeviceEntry::get(device).ok().map(|d| (d.device, d.queue))
}

impl BufferEntry {
    /// Buffer `id`'s entry, for Rust callers (`None`: no such buffer).
    pub fn lookup(id: u32) -> Option<Self> {
        registry::get::<BufferEntry>(id).ok()
    }

    fn map_state(&self) -> MapState {
        self.map.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set_map_state(&self, state: MapState) {
        *self.map.lock().unwrap_or_else(|e| e.into_inner()) = state;
    }
}

/// `GPUBufferDescriptor`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BufferDescriptor {
    #[serde(default)]
    label: String,
    size: u64,
    usage: u32,
    #[serde(default)]
    mapped_at_creation: bool,
}

/// The mapped range `[offset, offset + size)` of `entry`'s mapping, checked
/// as `getMappedRange` checks it.
fn mapped_range(
    entry: &BufferEntry,
    offset: u64,
    size: Option<u64>,
) -> Result<(u64, u64, bool), NativeError> {
    let MapState::Mapped {
        offset: start,
        size: len,
        write,
    } = entry.map_state()
    else {
        return Err(NativeError::operation(
            "getMappedRange: the buffer is not mapped",
        ));
    };
    let size = size.unwrap_or_else(|| (start + len).saturating_sub(offset));
    if !offset.is_multiple_of(8) || !size.is_multiple_of(4) {
        return Err(NativeError::operation(format!(
            "getMappedRange: offset {offset} must be a multiple of 8 and size {size} of 4"
        )));
    }
    // Checked: offset and size come from the caller (u64 in the Rust API).
    let end = offset.checked_add(size);
    if offset < start || end.is_none_or(|end| end > start + len) {
        return Err(NativeError::operation(format!(
            "getMappedRange: [{offset}, {offset} + {size}) is outside the mapped range [{start}, {})",
            start + len
        )));
    }
    Ok((offset, size, write))
}

/// The native half of `GPUBuffer`.
#[module]
pub struct WgpuBuffer;

#[module]
impl WgpuBuffer {
    /// `device.createBuffer(descriptor)`: the buffer's id. A RangeError for
    /// `mappedAtCreation` with a size that is not a multiple of 4.
    pub fn create(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let desc: BufferDescriptor = parse("GPUBufferDescriptor", descriptor)?;
            if desc.mapped_at_creation && !desc.size.is_multiple_of(4) {
                return Err(NativeError::new(
                    crate::error::ErrorName::RangeError,
                    format!(
                        "createBuffer: mappedAtCreation needs a size that is a multiple of 4, not {}",
                        desc.size
                    ),
                ));
            }
            // A bit WebGPU does not define makes the buffer invalid (a
            // validation error), not one of wgpu's native usages.
            let usage = if desc.usage & !SPEC_USAGE != 0 {
                entry.report(
                    GpuErrorKind::Validation,
                    format!(
                        "createBuffer: usage 0x{:x} has bits WebGPU does not define",
                        desc.usage
                    ),
                );
                wgpu::BufferUsages::empty()
            } else {
                wgpu::BufferUsages::from_bits_truncate(desc.usage)
            };
            let buffer = entry.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(desc.label.as_str()).filter(|l| !l.is_empty()),
                size: desc.size,
                usage,
                mapped_at_creation: desc.mapped_at_creation,
            });
            let map = if desc.mapped_at_creation {
                MapState::Mapped {
                    offset: 0,
                    size: desc.size,
                    write: true,
                }
            } else {
                MapState::Unmapped
            };
            Ok(registry::insert(BufferEntry {
                buffer,
                device,
                map: Arc::new(Mutex::new(map)),
            }))
        })
    }

    /// `buffer.mapAsync(mode, offset, size)`: a ticket to `Wgpu.wait` on.
    /// It settles with an `OperationError` when the map fails (the device
    /// reports why as a validation error), an `AbortError` when `unmap()`
    /// or `destroy()` comes first.
    pub fn map_async(
        buffer: u32,
        mode: u32,
        offset: Option<f64>,
        size: Option<f64>,
    ) -> Result<u32, String> {
        guard(|| {
            let entry = registry::get::<BufferEntry>(buffer)?;
            let device = DeviceEntry::get(entry.device)?;
            let offset = offset.unwrap_or(0.0) as u64;
            let total = entry.buffer.size();
            let size = size.map_or_else(|| total.saturating_sub(offset), |s| s as u64);
            let fail = |message: String| {
                device.report(GpuErrorKind::Validation, message.clone());
                Ok(pending::settled(
                    &device.device,
                    Err(NativeError::operation(message)),
                ))
            };
            if !matches!(entry.map_state(), MapState::Unmapped) {
                return Ok(pending::settled(
                    &device.device,
                    Err(NativeError::operation(
                        "mapAsync: the buffer is already mapped or mapping",
                    )),
                ));
            }
            let usage = entry.buffer.usage();
            let wgpu_mode = match mode {
                MAP_READ if usage.contains(wgpu::BufferUsages::MAP_READ) => wgpu::MapMode::Read,
                MAP_WRITE if usage.contains(wgpu::BufferUsages::MAP_WRITE) => wgpu::MapMode::Write,
                _ => {
                    return fail(format!(
                        "mapAsync: mode {mode} does not match the buffer's usage 0x{:x}",
                        usage.bits()
                    ));
                }
            };
            if !offset.is_multiple_of(8)
                || !size.is_multiple_of(4)
                || offset.checked_add(size).is_none_or(|end| end > total)
            {
                return fail(format!(
                    "mapAsync: [{offset}, {offset} + {size}) must start at a multiple of 8, have a size that is a multiple of 4 and fit in {total} bytes"
                ));
            }
            // The queue's writes so far (`writeBuffer`) come before the map,
            // as on WebGPU's queue timeline; wgpu holds them until a submit.
            device.queue.submit([]);
            let (ticket, slot) = pending::start(&device.device);
            entry.set_map_state(MapState::Pending(Arc::clone(&slot)));
            let map = Arc::clone(&entry.map);
            let write = wgpu_mode == wgpu::MapMode::Write;
            entry
                .buffer
                .map_async(wgpu_mode, offset..offset + size, move |result| {
                    let mut state = map.lock().unwrap_or_else(|e| e.into_inner());
                    // Still ours: not unmapped or destroyed meanwhile.
                    if matches!(&*state, MapState::Pending(s) if Arc::ptr_eq(s, &slot)) {
                        match result {
                            Ok(()) => {
                                *state = MapState::Mapped {
                                    offset,
                                    size,
                                    write,
                                };
                                pending::fill(&slot, Ok(()));
                            }
                            Err(e) => {
                                *state = MapState::Unmapped;
                                pending::fill(
                                    &slot,
                                    Err(NativeError::operation(format!("mapAsync: {e}"))),
                                );
                            }
                        }
                    }
                });
            Ok(ticket)
        })
    }

    /// `buffer.getMappedRange(offset, size)`: a copy of the mapped bytes.
    /// An `OperationError` when the buffer is not mapped or the range is
    /// outside the mapping or misaligned.
    pub fn get_mapped_range(
        buffer: u32,
        offset: Option<f64>,
        size: Option<f64>,
    ) -> Result<Vec<u8>, String> {
        guard(|| {
            let entry = registry::get::<BufferEntry>(buffer)?;
            let (offset, size, _) =
                mapped_range(&entry, offset.unwrap_or(0.0) as u64, size.map(|s| s as u64))?;
            if size == 0 {
                return Ok(Vec::new());
            }
            let view = entry.buffer.slice(offset..offset + size).get_mapped_range();
            Ok(view.to_vec())
        })
    }

    /// Writes `bytes` at `offset` of a buffer mapped for writing (what
    /// `unmap()` copies back from each range `getMappedRange` gave).
    pub fn write_mapped(buffer: u32, offset: f64, bytes: Bytes) -> Result<(), String> {
        guard(|| {
            let entry = registry::get::<BufferEntry>(buffer)?;
            let offset = offset as u64;
            let (offset, size, write) = mapped_range(&entry, offset, Some(bytes.0.len() as u64))?;
            if !write {
                return Err(NativeError::operation("the buffer is mapped for reading"));
            }
            if size > 0 {
                entry
                    .buffer
                    .slice(offset..offset + size)
                    .get_mapped_range_mut()
                    .copy_from_slice(&bytes.0);
            }
            Ok(())
        })
    }

    /// `buffer.unmap()`: ends the mapping; a pending `mapAsync` settles
    /// with an `AbortError`.
    pub fn unmap(buffer: u32) -> Result<(), String> {
        guard(|| {
            let entry = registry::get::<BufferEntry>(buffer)?;
            match entry.map_state() {
                MapState::Unmapped => {}
                MapState::Pending(slot) => {
                    entry.set_map_state(MapState::Unmapped);
                    pending::fill(
                        &slot,
                        Err(NativeError::new(
                            crate::error::ErrorName::AbortError,
                            "mapAsync: the buffer was unmapped before the mapping finished",
                        )),
                    );
                    entry.buffer.unmap();
                }
                MapState::Mapped { .. } => {
                    entry.set_map_state(MapState::Unmapped);
                    entry.buffer.unmap();
                }
            }
            Ok(())
        })
    }

    /// `buffer.mapState`: `"unmapped"`, `"pending"` or `"mapped"`.
    pub fn map_state(buffer: u32) -> Result<String, String> {
        guard(|| {
            Ok(match registry::get::<BufferEntry>(buffer)?.map_state() {
                MapState::Unmapped => "unmapped",
                MapState::Pending(_) => "pending",
                MapState::Mapped { .. } => "mapped",
            }
            .into())
        })
    }

    /// `buffer.destroy()`: unmaps it (a pending map settles with an
    /// `AbortError`) and frees its memory; using it after is a validation
    /// error.
    pub fn destroy(buffer: u32) -> Result<(), String> {
        Self::unmap(buffer)?;
        guard(|| {
            registry::get::<BufferEntry>(buffer)?.buffer.destroy();
            Ok(())
        })
    }
}
