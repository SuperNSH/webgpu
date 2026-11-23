//! `GPUBindGroup`: `device.createBindGroup`.
//!
//! An entry's `resource` is any `GPUBindingResource`: a `GPUSampler`, a
//! `GPUTextureView`, a `GPUTexture` (its default view), a `GPUBuffer` (the
//! whole buffer), a `GPUBufferBinding` (`{ buffer, offset, size }`), or a
//! `GPUExternalTexture`. They cross as ids (`wire`), and the registry tells
//! the kind.

use std::num::NonZeroU64;

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::{
        bind_group_layout::BindGroupLayoutEntry, buffer::BufferEntry, device::DeviceEntry,
        external_texture::ExternalTextureEntry, sampler::SamplerEntry, texture::TextureEntry,
        texture_view::TextureViewEntry,
    },
    convert::parse,
    error::{GpuErrorKind, NativeError, guard},
    registry::{self, Id},
};

/// A bind group in the registry.
#[derive(Clone)]
pub struct BindGroupEntry {
    /// The bind group.
    pub bind_group: wgpu::BindGroup,
    /// Its device.
    pub device: Id,
    /// `false` for the spec's invalid bind group (made from an invalid
    /// layout or sampler).
    pub valid: bool,
}

impl BindGroupEntry {
    /// Bind group `id`'s entry, for Rust callers (`None`: no such group).
    pub fn lookup(id: u32) -> Option<Self> {
        registry::get::<BindGroupEntry>(id).ok()
    }
}

/// `GPUBindingResource` as it crosses.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Resource {
    /// A sampler, view, texture, buffer or external texture.
    Object(Id),
    /// `GPUBufferBinding`.
    Buffer {
        buffer: Id,
        #[serde(default)]
        offset: u64,
        size: Option<u64>,
    },
}

/// `GPUBindGroupEntry`.
#[derive(Debug, Deserialize)]
struct Entry {
    binding: u32,
    resource: Resource,
}

/// `GPUBindGroupDescriptor`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BindGroupDescriptor {
    #[serde(default)]
    label: String,
    layout: Id,
    entries: Vec<Entry>,
}

/// A resource, resolved to the wgpu objects it binds.
enum Bound {
    Sampler(SamplerEntry),
    View(wgpu::TextureView),
    Buffer {
        buffer: wgpu::Buffer,
        offset: u64,
        size: Option<NonZeroU64>,
    },
    External(wgpu::ExternalTexture),
}

impl Bound {
    /// What `resource` binds; a `TypeError` for an object that is not a
    /// `GPUBindingResource`.
    fn of(resource: &Resource) -> Result<Self, NativeError> {
        match resource {
            Resource::Buffer {
                buffer,
                offset,
                size,
            } => Ok(Bound::Buffer {
                buffer: registry::get::<BufferEntry>(*buffer)?.buffer,
                offset: *offset,
                size: size.and_then(NonZeroU64::new),
            }),
            Resource::Object(id) => {
                if let Ok(s) = registry::get::<SamplerEntry>(*id) {
                    Ok(Bound::Sampler(s))
                } else if let Ok(v) = registry::get::<TextureViewEntry>(*id) {
                    Ok(Bound::View(v.view))
                } else if let Ok(t) = registry::get::<TextureEntry>(*id) {
                    Ok(Bound::View(TextureViewEntry::default_of(&t).view))
                } else if let Ok(b) = registry::get::<BufferEntry>(*id) {
                    Ok(Bound::Buffer {
                        buffer: b.buffer,
                        offset: 0,
                        size: None,
                    })
                } else if let Ok(e) = registry::get::<ExternalTextureEntry>(*id) {
                    Ok(Bound::External(e.external_texture))
                } else if registry::contains(*id) {
                    Err(NativeError::type_error(format!(
                        "object {id} is not a GPUBindingResource"
                    )))
                } else {
                    Err(NativeError::type_error(format!(
                        "no object {id} (released?)"
                    )))
                }
            }
        }
    }

    fn resource(&self) -> wgpu::BindingResource<'_> {
        match self {
            Bound::Sampler(s) => wgpu::BindingResource::Sampler(&s.sampler),
            Bound::View(v) => wgpu::BindingResource::TextureView(v),
            Bound::Buffer {
                buffer,
                offset,
                size,
            } => wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: *offset,
                size: *size,
            }),
            Bound::External(t) => wgpu::BindingResource::ExternalTexture(t),
        }
    }
}

/// The native half of `GPUBindGroup`.
#[module]
pub struct WgpuBindGroup;

#[module]
impl WgpuBindGroup {
    /// `device.createBindGroup(descriptor)`: the bind group's id. What the
    /// spec finds invalid (a resource that does not match its layout entry,
    /// a binding the layout lacks, an offset out of range) is a validation
    /// error and an invalid bind group.
    pub fn create(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let desc: BindGroupDescriptor = parse("GPUBindGroupDescriptor", descriptor)?;
            let layout = registry::get::<BindGroupLayoutEntry>(desc.layout)?;
            let bound = desc
                .entries
                .iter()
                .map(|e| Bound::of(&e.resource))
                .collect::<Result<Vec<_>, _>>()?;
            let mut valid = layout.valid;
            // wgpu reads size 0 as "the rest"; the spec has it invalid.
            if let Some(e) = desc
                .entries
                .iter()
                .find(|e| matches!(e.resource, Resource::Buffer { size: Some(0), .. }))
            {
                valid = false;
                entry.report(
                    GpuErrorKind::Validation,
                    format!(
                        "createBindGroup: the buffer binding at {} has size 0",
                        e.binding
                    ),
                );
            }
            if !layout.valid {
                entry.report(
                    GpuErrorKind::Validation,
                    "createBindGroup: its layout is invalid",
                );
            }
            if let Some(e) = desc
                .entries
                .iter()
                .zip(&bound)
                .find(|(_, b)| matches!(b, Bound::Sampler(s) if !s.valid))
            {
                valid = false;
                entry.report(
                    GpuErrorKind::Validation,
                    format!(
                        "createBindGroup: the sampler at binding {} is invalid",
                        e.0.binding
                    ),
                );
            }
            let entries: Vec<wgpu::BindGroupEntry<'_>> = desc
                .entries
                .iter()
                .zip(&bound)
                .map(|(e, b)| wgpu::BindGroupEntry {
                    binding: e.binding,
                    resource: b.resource(),
                })
                .collect();
            let bind_group = entry.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(desc.label.as_str()).filter(|l| !l.is_empty()),
                layout: &layout.layout,
                entries: &entries,
            });
            Ok(registry::insert(BindGroupEntry {
                bind_group,
                device,
                valid,
            }))
        })
    }
}
