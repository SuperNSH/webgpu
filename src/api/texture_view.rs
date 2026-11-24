//! `GPUTextureView`: `texture.createView`.

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::{
        device::DeviceEntry,
        texture::{TextureEntry, spec_format},
    },
    convert::parse,
    error::{GpuErrorKind, NativeError, guard},
    registry::{self, Id},
};

/// The usage bits WebGPU defines (`GPUTextureUsage`).
const SPEC_USAGE: u32 = 0x1f;

/// A texture view in the registry.
#[derive(Clone)]
pub struct TextureViewEntry {
    /// The view.
    pub view: wgpu::TextureView,
    /// Its texture's device.
    pub device: Id,
    /// The canvas context whose surface its texture is a frame of.
    pub frame_of: Option<u32>,
}

/// `GPUTextureViewDescriptor`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextureViewDescriptor {
    #[serde(default)]
    label: String,
    format: Option<wgpu::TextureFormat>,
    dimension: Option<wgpu::TextureViewDimension>,
    #[serde(default)]
    usage: u32,
    #[serde(default)]
    aspect: wgpu::TextureAspect,
    #[serde(default)]
    base_mip_level: u32,
    mip_level_count: Option<u32>,
    #[serde(default)]
    base_array_layer: u32,
    array_layer_count: Option<u32>,
}

impl TextureViewEntry {
    /// View `id`'s entry, for Rust callers (`None`: no such view).
    pub fn lookup(id: u32) -> Option<Self> {
        registry::get::<TextureViewEntry>(id).ok()
    }

    /// The default view of texture `texture` (a `GPUTexture` bound where
    /// a view is expected: the spec's `texture.createView()`).
    pub(crate) fn default_of(texture: &TextureEntry) -> Self {
        TextureViewEntry {
            view: texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
            device: texture.device,
            frame_of: texture.frame_of,
        }
    }
}

/// Lets go of context `context`'s past frames, the textures and the views JS
/// may hold until it collects them: each becomes a destroyed texture or a
/// view of one, so using it stays the validation error an expired canvas
/// texture is. A DX12 swap chain cannot be resized while anything holds one
/// of its buffers (`ResizeBuffers`: `DXGI_ERROR_INVALID_CALL`), and a
/// surface's texture holds one past its present (wgpu-core's clear view).
pub(crate) fn expire_frames(context: u32) {
    let textures = registry::find(|t: &TextureEntry| {
        (t.frame_of == Some(context)).then(|| {
            let texture = &t.texture;
            let size = texture.size();
            (t.device, size, texture.format(), texture.usage())
        })
    });
    for (id, (device, size, format, usage)) in textures {
        let Ok(entry) = DeviceEntry::get(device) else {
            registry::release(id);
            continue;
        };
        let texture = expired(&entry.device, size, format, usage);
        texture.destroy();
        registry::replace(
            id,
            TextureEntry {
                texture,
                device,
                frame_of: None,
            },
        );
    }
    let views = registry::find(|v: &TextureViewEntry| {
        (v.frame_of == Some(context)).then(|| (v.device, v.view.texture().format()))
    });
    for (id, (device, format)) in views {
        let Ok(entry) = DeviceEntry::get(device) else {
            registry::release(id);
            continue;
        };
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let texture = expired(&entry.device, size, format, usage);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        texture.destroy();
        registry::replace(
            id,
            TextureViewEntry {
                view,
                device,
                frame_of: None,
            },
        );
    }
}

/// What stands in for an expired frame.
fn expired(
    device: &wgpu::Device,
    size: wgpu::Extent3d,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("an expired canvas frame"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

/// The native half of `GPUTextureView`.
#[module]
pub struct WgpuTextureView;

#[module]
impl WgpuTextureView {
    /// `texture.createView(descriptor)`: the view's id.
    pub fn create(texture: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let entry = registry::get::<TextureEntry>(texture)?;
            let desc: TextureViewDescriptor = parse("GPUTextureViewDescriptor", descriptor)?;
            let format = desc.format.map(spec_format).transpose()?;
            if matches!(
                desc.aspect,
                wgpu::TextureAspect::Plane0
                    | wgpu::TextureAspect::Plane1
                    | wgpu::TextureAspect::Plane2
            ) {
                return Err(NativeError::type_error(
                    "aspect: planes are not a valid GPUTextureAspect",
                ));
            }
            let usage = if desc.usage & !SPEC_USAGE != 0 {
                DeviceEntry::get(entry.device)?.report(
                    GpuErrorKind::Validation,
                    format!(
                        "createView: usage 0x{:x} has bits WebGPU does not define",
                        desc.usage
                    ),
                );
                Some(wgpu::TextureUsages::empty())
            } else {
                (desc.usage != 0).then(|| wgpu::TextureUsages::from_bits_truncate(desc.usage))
            };
            let view = entry.texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(desc.label.as_str()).filter(|l| !l.is_empty()),
                format,
                dimension: desc.dimension,
                usage,
                aspect: desc.aspect,
                base_mip_level: desc.base_mip_level,
                mip_level_count: desc.mip_level_count,
                base_array_layer: desc.base_array_layer,
                array_layer_count: desc.array_layer_count,
            });
            Ok(registry::insert(TextureViewEntry {
                view,
                device: entry.device,
                frame_of: entry.frame_of,
            }))
        })
    }
}
