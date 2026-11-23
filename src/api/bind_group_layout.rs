//! `GPUBindGroupLayout`: `device.createBindGroupLayout`; pipelines'
//! `getBindGroupLayout` makes entries too.

use std::num::NonZeroU64;

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::{device::DeviceEntry, texture::spec_format},
    convert::parse,
    error::{GpuErrorKind, guard},
    registry::{self, Id},
};

/// A bind group layout in the registry.
#[derive(Clone)]
pub struct BindGroupLayoutEntry {
    /// The layout.
    pub layout: wgpu::BindGroupLayout,
    /// Its device.
    pub device: Id,
    /// `false` for the spec's invalid layout (its descriptor failed
    /// validation): what uses it is invalid too.
    pub valid: bool,
}

impl BindGroupLayoutEntry {
    /// Layout `id`'s entry, for Rust callers (`None`: no such layout).
    pub fn lookup(id: u32) -> Option<Self> {
        registry::get::<BindGroupLayoutEntry>(id).ok()
    }
}

/// `GPUShaderStage` bits.
const SPEC_STAGES: u32 = 0x7;

/// `GPUBufferBindingType`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum BufferBindingType {
    #[default]
    Uniform,
    Storage,
    ReadOnlyStorage,
}

/// `GPUBufferBindingLayout`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct BufferLayout {
    #[serde(rename = "type")]
    ty: BufferBindingType,
    has_dynamic_offset: bool,
    min_binding_size: u64,
}

/// `GPUSamplerBindingLayout`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SamplerLayout {
    #[serde(rename = "type")]
    ty: Option<wgpu::SamplerBindingType>,
}

/// `GPUTextureSampleType`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SampleType {
    #[default]
    Float,
    UnfilterableFloat,
    Depth,
    Sint,
    Uint,
}

/// `GPUTextureBindingLayout`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct TextureLayout {
    sample_type: SampleType,
    view_dimension: Option<wgpu::TextureViewDimension>,
    multisampled: bool,
}

/// `GPUStorageTextureAccess`: the spec's three (wgpu's `atomic` is
/// native-only).
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum StorageAccess {
    #[default]
    WriteOnly,
    ReadOnly,
    ReadWrite,
}

/// `GPUStorageTextureBindingLayout`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageTextureLayout {
    #[serde(default)]
    access: StorageAccess,
    format: wgpu::TextureFormat,
    view_dimension: Option<wgpu::TextureViewDimension>,
}

/// `GPUExternalTextureBindingLayout` (no members).
#[derive(Debug, Default, Deserialize)]
struct ExternalTextureLayout {}

/// `GPUBindGroupLayoutEntry`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LayoutEntry {
    binding: u32,
    visibility: u32,
    buffer: Option<BufferLayout>,
    sampler: Option<SamplerLayout>,
    texture: Option<TextureLayout>,
    storage_texture: Option<StorageTextureLayout>,
    external_texture: Option<ExternalTextureLayout>,
}

/// `GPUBindGroupLayoutDescriptor`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LayoutDescriptor {
    #[serde(default)]
    label: String,
    entries: Vec<LayoutEntry>,
}

impl LayoutEntry {
    /// As wgpu's, or why the spec finds it invalid (a validation error).
    fn to_wgpu(
        &self,
    ) -> Result<Result<wgpu::BindGroupLayoutEntry, String>, crate::error::NativeError> {
        let binding = self.binding;
        if self.visibility & !SPEC_STAGES != 0 {
            return Ok(Err(format!(
                "binding {binding}: visibility 0x{:x} has bits GPUShaderStage does not define",
                self.visibility
            )));
        }
        let set = [
            self.buffer.is_some(),
            self.sampler.is_some(),
            self.texture.is_some(),
            self.storage_texture.is_some(),
            self.external_texture.is_some(),
        ]
        .into_iter()
        .filter(|b| *b)
        .count();
        if set != 1 {
            return Ok(Err(format!(
                "binding {binding}: exactly one of buffer, sampler, texture, storageTexture and externalTexture must be given, not {set}"
            )));
        }
        let ty = if let Some(b) = &self.buffer {
            wgpu::BindingType::Buffer {
                ty: match b.ty {
                    BufferBindingType::Uniform => wgpu::BufferBindingType::Uniform,
                    BufferBindingType::Storage => {
                        wgpu::BufferBindingType::Storage { read_only: false }
                    }
                    BufferBindingType::ReadOnlyStorage => {
                        wgpu::BufferBindingType::Storage { read_only: true }
                    }
                },
                has_dynamic_offset: b.has_dynamic_offset,
                min_binding_size: NonZeroU64::new(b.min_binding_size),
            }
        } else if let Some(s) = &self.sampler {
            wgpu::BindingType::Sampler(s.ty.unwrap_or(wgpu::SamplerBindingType::Filtering))
        } else if let Some(t) = &self.texture {
            wgpu::BindingType::Texture {
                sample_type: match t.sample_type {
                    SampleType::Float => wgpu::TextureSampleType::Float { filterable: true },
                    SampleType::UnfilterableFloat => {
                        wgpu::TextureSampleType::Float { filterable: false }
                    }
                    SampleType::Depth => wgpu::TextureSampleType::Depth,
                    SampleType::Sint => wgpu::TextureSampleType::Sint,
                    SampleType::Uint => wgpu::TextureSampleType::Uint,
                },
                view_dimension: t.view_dimension.unwrap_or(wgpu::TextureViewDimension::D2),
                multisampled: t.multisampled,
            }
        } else if let Some(t) = &self.storage_texture {
            wgpu::BindingType::StorageTexture {
                access: match t.access {
                    StorageAccess::WriteOnly => wgpu::StorageTextureAccess::WriteOnly,
                    StorageAccess::ReadOnly => wgpu::StorageTextureAccess::ReadOnly,
                    StorageAccess::ReadWrite => wgpu::StorageTextureAccess::ReadWrite,
                },
                format: spec_format(t.format)?,
                view_dimension: t.view_dimension.unwrap_or(wgpu::TextureViewDimension::D2),
            }
        } else {
            wgpu::BindingType::ExternalTexture
        };
        Ok(Ok(wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::from_bits_truncate(self.visibility),
            ty,
            count: None,
        }))
    }
}

/// The native half of `GPUBindGroupLayout`.
#[module]
pub struct WgpuBindGroupLayout;

#[module]
impl WgpuBindGroupLayout {
    /// `device.createBindGroupLayout(descriptor)`: the layout's id. An
    /// entry the spec finds invalid is a validation error and an invalid
    /// layout, as wgpu's own checks (duplicate bindings, limits, features
    /// such as `externalTexture`'s) are.
    pub fn create(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let desc: LayoutDescriptor = parse("GPUBindGroupLayoutDescriptor", descriptor)?;
            let mut entries = Vec::with_capacity(desc.entries.len());
            let mut problem = None;
            for e in &desc.entries {
                match e.to_wgpu()? {
                    Ok(e) => entries.push(e),
                    Err(message) => {
                        problem.get_or_insert(message);
                    }
                }
            }
            if let Some(message) = &problem {
                entry.report(
                    GpuErrorKind::Validation,
                    format!("createBindGroupLayout: {message}"),
                );
            }
            let layout = entry
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some(desc.label.as_str()).filter(|l| !l.is_empty()),
                    entries: if problem.is_some() { &[] } else { &entries },
                });
            Ok(registry::insert(BindGroupLayoutEntry {
                layout,
                device,
                valid: problem.is_none(),
            }))
        })
    }
}
