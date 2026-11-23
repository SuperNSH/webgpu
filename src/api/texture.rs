//! `GPUTexture`: `device.createTexture`, `destroy()`. Its views are
//! `texture_view.rs`'s.

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::device::DeviceEntry,
    convert::{Extent3d, parse},
    error::{GpuErrorKind, NativeError, guard},
    registry::{self, Id},
};

/// `format` if the spec names it (`GPUTextureFormat`); a `TypeError` (the
/// WebIDL enum check) for the formats wgpu reads but WebGPU does not have:
/// `r64uint`, `nv12`, `p010` and the ASTC HDR formats.
pub(crate) fn spec_format(format: wgpu::TextureFormat) -> Result<wgpu::TextureFormat, NativeError> {
    use wgpu::{AstcChannel, TextureFormat as F};
    match format {
        F::R64Uint | F::NV12 | F::P010 => Err(NativeError::type_error(format!(
            "'{format:?}' is not a valid GPUTextureFormat"
        ))),
        F::Astc {
            channel: AstcChannel::Hdr,
            ..
        } => Err(NativeError::type_error(
            "ASTC HDR formats are not valid GPUTextureFormats",
        )),
        other => Ok(other),
    }
}

/// The usage bits WebGPU defines (`GPUTextureUsage`); wgpu's share them.
const SPEC_USAGE: u32 = 0x1f;

/// A texture in the registry.
#[derive(Clone)]
pub struct TextureEntry {
    /// The texture.
    pub texture: wgpu::Texture,
    /// Its device.
    pub device: Id,
}

/// `GPUTextureDescriptor`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextureDescriptor {
    #[serde(default)]
    label: String,
    size: Extent3d,
    #[serde(default = "one")]
    mip_level_count: u32,
    #[serde(default = "one")]
    sample_count: u32,
    #[serde(default = "d2")]
    dimension: wgpu::TextureDimension,
    format: wgpu::TextureFormat,
    usage: u32,
    #[serde(default)]
    view_formats: Vec<wgpu::TextureFormat>,
    /// Compatibility mode only; a core device (wgpu's) ignores it.
    #[serde(default)]
    #[allow(dead_code)]
    texture_binding_view_dimension: Option<wgpu::TextureViewDimension>,
}

fn one() -> u32 {
    1
}

fn d2() -> wgpu::TextureDimension {
    wgpu::TextureDimension::D2
}

impl TextureEntry {
    /// Texture `id`'s entry, for Rust callers (`None`: no such texture).
    pub fn lookup(id: u32) -> Option<Self> {
        registry::get::<TextureEntry>(id).ok()
    }
}

/// The native half of `GPUTexture`.
#[module]
pub struct WgpuTexture;

#[module]
impl WgpuTexture {
    /// `device.createTexture(descriptor)`: the texture's id.
    pub fn create(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let desc: TextureDescriptor = parse("GPUTextureDescriptor", descriptor)?;
            let format = spec_format(desc.format)?;
            for view_format in &desc.view_formats {
                spec_format(*view_format)?;
            }
            let usage = if desc.usage & !SPEC_USAGE != 0 {
                entry.report(
                    GpuErrorKind::Validation,
                    format!(
                        "createTexture: usage 0x{:x} has bits WebGPU does not define",
                        desc.usage
                    ),
                );
                wgpu::TextureUsages::empty()
            } else {
                wgpu::TextureUsages::from_bits_truncate(desc.usage)
            };
            let texture = entry.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(desc.label.as_str()).filter(|l| !l.is_empty()),
                size: desc.size.to_wgpu()?,
                mip_level_count: desc.mip_level_count,
                sample_count: desc.sample_count,
                dimension: desc.dimension,
                format,
                usage,
                view_formats: &desc.view_formats,
            });
            Ok(registry::insert(TextureEntry { texture, device }))
        })
    }

    /// `texture.destroy()`.
    pub fn destroy(texture: u32) -> Result<(), String> {
        guard(|| {
            registry::get::<TextureEntry>(texture)?.texture.destroy();
            Ok(())
        })
    }
}
