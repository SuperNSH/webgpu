//! `GPUSampler`: `device.createSampler`.

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::device::DeviceEntry,
    convert::parse,
    error::{GpuErrorKind, guard},
    registry::{self, Id},
};

/// A sampler in the registry.
#[derive(Clone)]
pub struct SamplerEntry {
    /// The sampler.
    pub sampler: wgpu::Sampler,
    /// Its device.
    pub device: Id,
    /// Whether it is a comparison sampler (`compare` was given).
    pub comparison: bool,
    /// `false` for the spec's invalid sampler (its descriptor failed
    /// validation): a bind group using it is invalid too.
    pub valid: bool,
}

impl SamplerEntry {
    /// Sampler `id`'s entry, for Rust callers (`None`: no such sampler).
    pub fn lookup(id: u32) -> Option<Self> {
        registry::get::<SamplerEntry>(id).ok()
    }
}

/// `GPUAddressMode`: the spec's three (wgpu's clamp-to-border and
/// clamp-to-zero are native-only, so not accepted).
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum AddressMode {
    #[default]
    ClampToEdge,
    Repeat,
    MirrorRepeat,
}

impl AddressMode {
    fn to_wgpu(self) -> wgpu::AddressMode {
        match self {
            AddressMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            AddressMode::Repeat => wgpu::AddressMode::Repeat,
            AddressMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
        }
    }
}

/// `GPUSamplerDescriptor`, with the spec's defaults.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SamplerDescriptor {
    label: String,
    address_mode_u: AddressMode,
    address_mode_v: AddressMode,
    address_mode_w: AddressMode,
    mag_filter: wgpu::FilterMode,
    min_filter: wgpu::FilterMode,
    mipmap_filter: wgpu::MipmapFilterMode,
    lod_min_clamp: f32,
    lod_max_clamp: f32,
    compare: Option<wgpu::CompareFunction>,
    /// `[Clamp] unsigned short`: clamped to 0..=65535 by TSX.
    max_anisotropy: u16,
}

impl Default for SamplerDescriptor {
    fn default() -> Self {
        Self {
            label: String::new(),
            address_mode_u: AddressMode::default(),
            address_mode_v: AddressMode::default(),
            address_mode_w: AddressMode::default(),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            compare: None,
            max_anisotropy: 1,
        }
    }
}

/// What the spec's "validating GPUSamplerDescriptor" finds wrong with
/// `desc`, if anything (wgpu checks some of it, not all, and not with the
/// spec's rules for `maxAnisotropy`).
fn invalid(desc: &SamplerDescriptor) -> Option<String> {
    if desc.lod_min_clamp < 0.0 || desc.lod_max_clamp < desc.lod_min_clamp {
        return Some(format!(
            "createSampler: lodMinClamp {} must be >= 0 and lodMaxClamp {} >= lodMinClamp",
            desc.lod_min_clamp, desc.lod_max_clamp
        ));
    }
    if desc.max_anisotropy < 1 {
        return Some("createSampler: maxAnisotropy must be at least 1".into());
    }
    if desc.max_anisotropy > 1
        && (desc.mag_filter != wgpu::FilterMode::Linear
            || desc.min_filter != wgpu::FilterMode::Linear
            || desc.mipmap_filter != wgpu::MipmapFilterMode::Linear)
    {
        return Some(format!(
            "createSampler: maxAnisotropy {} needs magFilter, minFilter and mipmapFilter \"linear\"",
            desc.max_anisotropy
        ));
    }
    None
}

/// The native half of `GPUSampler`.
#[module]
pub struct WgpuSampler;

#[module]
impl WgpuSampler {
    /// `device.createSampler(descriptor)`: the sampler's id. An invalid
    /// descriptor is a validation error and an invalid sampler, as in the
    /// spec.
    pub fn create(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let desc: SamplerDescriptor = parse("GPUSamplerDescriptor", descriptor)?;
            let problem = invalid(&desc);
            if let Some(message) = &problem {
                entry.report(GpuErrorKind::Validation, message.clone());
            }
            // The spec clamps maxAnisotropy to what the platform has; wgpu
            // clamps to 16.
            let anisotropy = if problem.is_some() {
                1
            } else {
                desc.max_anisotropy.min(16)
            };
            let sampler = entry.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(desc.label.as_str()).filter(|l| !l.is_empty()),
                address_mode_u: desc.address_mode_u.to_wgpu(),
                address_mode_v: desc.address_mode_v.to_wgpu(),
                address_mode_w: desc.address_mode_w.to_wgpu(),
                mag_filter: desc.mag_filter,
                min_filter: desc.min_filter,
                mipmap_filter: desc.mipmap_filter,
                lod_min_clamp: if problem.is_some() {
                    0.0
                } else {
                    desc.lod_min_clamp
                },
                lod_max_clamp: if problem.is_some() {
                    32.0
                } else {
                    desc.lod_max_clamp
                },
                compare: desc.compare,
                anisotropy_clamp: anisotropy,
                border_color: None,
            });
            Ok(registry::insert(SamplerEntry {
                sampler,
                device,
                comparison: desc.compare.is_some(),
                valid: problem.is_none(),
            }))
        })
    }
}
