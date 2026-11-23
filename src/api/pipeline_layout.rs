//! `GPUPipelineLayout`: `device.createPipelineLayout`.

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::{bind_group_layout::BindGroupLayoutEntry, device::DeviceEntry},
    convert::parse,
    error::{GpuErrorKind, guard},
    registry::{self, Id},
};

/// A pipeline layout in the registry.
#[derive(Clone)]
pub struct PipelineLayoutEntry {
    /// The layout.
    pub layout: wgpu::PipelineLayout,
    /// Its device.
    pub device: Id,
}

impl PipelineLayoutEntry {
    /// Layout `id`'s entry, for Rust callers (`None`: no such layout).
    pub fn lookup(id: u32) -> Option<Self> {
        registry::get::<PipelineLayoutEntry>(id).ok()
    }
}

/// `GPUPipelineLayoutDescriptor`. A `null` (or `undefined`, or a hole) in
/// `bindGroupLayouts` is an empty group that keeps the later ones' indices.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PipelineLayoutDescriptor {
    #[serde(default)]
    label: String,
    bind_group_layouts: Vec<Option<Id>>,
    /// `immediateSize` (the spec's newer member; needs wgpu's `IMMEDIATES`).
    #[serde(default)]
    immediate_size: u32,
}

/// The native half of `GPUPipelineLayout`.
#[module]
pub struct WgpuPipelineLayout;

#[module]
impl WgpuPipelineLayout {
    /// `device.createPipelineLayout(descriptor)`: the layout's id. An
    /// invalid bind group layout in it is a validation error.
    pub fn create(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let desc: PipelineLayoutDescriptor = parse("GPUPipelineLayoutDescriptor", descriptor)?;
            let layouts = desc
                .bind_group_layouts
                .iter()
                .map(|id| id.map(registry::get::<BindGroupLayoutEntry>).transpose())
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(index) = layouts
                .iter()
                .position(|l| l.as_ref().is_some_and(|l| !l.valid))
            {
                entry.report(
                    GpuErrorKind::Validation,
                    format!("createPipelineLayout: bindGroupLayouts[{index}] is invalid"),
                );
            }
            let refs: Vec<Option<&wgpu::BindGroupLayout>> = layouts
                .iter()
                .map(|l| l.as_ref().map(|l| &l.layout))
                .collect();
            let layout = entry
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(desc.label.as_str()).filter(|l| !l.is_empty()),
                    bind_group_layouts: &refs,
                    immediate_size: desc.immediate_size,
                });
            Ok(registry::insert(PipelineLayoutEntry { layout, device }))
        })
    }
}
