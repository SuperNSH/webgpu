//! `GPUQuerySet`: `device.createQuerySet`, `destroy()`.
//!
//! wgpu's `QuerySet` has no `destroy`: `destroy()` marks the entry, and the
//! commands that use a query set (`beginOcclusionQuery`, `timestampWrites`,
//! `resolveQuerySet`: area commands) check [`QuerySetEntry::destroyed`]
//! and report the spec's validation error. The memory goes when the last
//! holder drops it.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::device::DeviceEntry,
    convert::parse,
    error::{GpuErrorKind, guard},
    registry::{self, Id},
};

/// The spec's most queries in one set.
const MAX_QUERIES: u32 = 4096;

/// A query set in the registry.
#[derive(Clone)]
pub struct QuerySetEntry {
    /// The query set.
    pub query_set: wgpu::QuerySet,
    /// Its device.
    pub device: Id,
    /// `"occlusion"` or `"timestamp"`.
    pub ty: wgpu::QueryType,
    /// How many queries it holds.
    pub count: u32,
    /// `false` for the spec's invalid set: using it is a validation error.
    pub valid: bool,
    /// Set by `destroy()`.
    pub(crate) destroyed: Arc<AtomicBool>,
}

impl QuerySetEntry {
    /// Query set `id`'s entry, for Rust callers (`None`: no such set).
    pub fn lookup(id: u32) -> Option<Self> {
        registry::get::<QuerySetEntry>(id).ok()
    }

    /// Whether `destroy()` was called: using it is a validation error.
    pub fn destroyed(&self) -> bool {
        self.destroyed.load(Ordering::Relaxed)
    }
}

/// `GPUQueryType`.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum QueryType {
    Occlusion,
    Timestamp,
}

/// `GPUQuerySetDescriptor`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuerySetDescriptor {
    #[serde(default)]
    label: String,
    #[serde(rename = "type")]
    ty: QueryType,
    count: u32,
}

/// The native half of `GPUQuerySet`.
#[module]
pub struct WgpuQuerySet;

#[module]
impl WgpuQuerySet {
    /// `device.createQuerySet(descriptor)`: the set's id. More than 4096
    /// queries, or `"timestamp"` on a device without `timestamp-query`, is
    /// a validation error and an invalid set (wgpu's checks and the
    /// spec's).
    pub fn create(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let desc: QuerySetDescriptor = parse("GPUQuerySetDescriptor", descriptor)?;
            let ty = match desc.ty {
                QueryType::Occlusion => wgpu::QueryType::Occlusion,
                QueryType::Timestamp => wgpu::QueryType::Timestamp,
            };
            let problem = if desc.count > MAX_QUERIES {
                Some(format!(
                    "createQuerySet: count {} is more than {MAX_QUERIES}",
                    desc.count
                ))
            } else if matches!(desc.ty, QueryType::Timestamp)
                && !entry
                    .device
                    .features()
                    .contains(wgpu::Features::TIMESTAMP_QUERY)
            {
                Some(
                    "createQuerySet: \"timestamp\" queries need the \"timestamp-query\" feature"
                        .to_owned(),
                )
            } else {
                None
            };
            // An invalid set still gets a (small, occlusion) wgpu object, so
            // the error is reported once, in the spec's words.
            let (wgpu_ty, wgpu_count) = match &problem {
                Some(message) => {
                    entry.report(GpuErrorKind::Validation, message.clone());
                    (wgpu::QueryType::Occlusion, 1)
                }
                None => (ty, desc.count.max(1)),
            };
            let query_set = entry.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some(desc.label.as_str()).filter(|l| !l.is_empty()),
                ty: wgpu_ty,
                count: wgpu_count,
            });
            Ok(registry::insert(QuerySetEntry {
                query_set,
                device,
                ty,
                count: desc.count,
                valid: problem.is_none(),
                destroyed: Arc::default(),
            }))
        })
    }

    /// `querySet.destroy()`: using it after is a validation error.
    pub fn destroy(query_set: u32) -> Result<(), String> {
        guard(|| {
            registry::get::<QuerySetEntry>(query_set)?
                .destroyed
                .store(true, Ordering::Relaxed);
            Ok(())
        })
    }
}
