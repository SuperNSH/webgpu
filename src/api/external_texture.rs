//! `GPUExternalTexture`: `device.importExternalTexture`.
//!
//! The spec's sources (`HTMLVideoElement`, `VideoFrame`) do not exist in
//! NSH, and react-native-webgpu's `NativeBuffer` source is the
//! react-parity area's. What wgpu can do is react-native-webgpu's other
//! source, a `GPUTexture` (`rgba8unorm` or `bgra8unorm`, with
//! `TEXTURE_BINDING`): wgpu 29's `ExternalTexture` with one RGBA plane,
//! with react-native-webgpu's `cropOrigin`/`cropSize`, `mirrored` and
//! `rotation` (clockwise, after the mirror) baked into its sample and load
//! transforms, and `colorSpace: "display-p3"` into its gamut conversion.
//! It needs wgpu's `EXTERNAL_TEXTURE` feature on the device; without it,
//! importing is `ERR_WEBGPU_UNSUPPORTED`.

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::{command_encoder::label, device::DeviceEntry, texture::TextureEntry},
    convert::parse,
    error::{NativeError, guard},
    registry::{self, Id},
};

/// A `GPUExternalTexture` in the registry.
#[derive(Clone)]
pub struct ExternalTextureEntry {
    /// The wgpu object, for a bind group's `externalTexture` entry
    /// (`wgpu::BindingResource::ExternalTexture`).
    pub external_texture: wgpu::ExternalTexture,
    /// Its device.
    pub device: Id,
}

/// `GPUOrigin2D` / `GPUExtent2D`: `[a, b]` or a dictionary.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Pair {
    Seq(Vec<u32>),
    // Before `Origin`, whose members all default (it would take any
    // dictionary).
    Extent {
        width: u32,
        #[serde(default = "one")]
        height: u32,
    },
    Origin {
        #[serde(default)]
        x: u32,
        #[serde(default)]
        y: u32,
    },
}

fn one() -> u32 {
    1
}

impl Pair {
    fn get(&self, default_second: u32) -> Result<(u32, u32), NativeError> {
        match self {
            Pair::Seq(v) if v.len() <= 2 => Ok((
                v.first().copied().unwrap_or(0),
                v.get(1).copied().unwrap_or(default_second),
            )),
            Pair::Seq(v) => Err(NativeError::type_error(format!(
                "a 2D origin or extent has at most 2 members, not {}",
                v.len()
            ))),
            Pair::Origin { x, y } => Ok((*x, *y)),
            Pair::Extent { width, height } => Ok((*width, *height)),
        }
    }
}

/// `GPUExternalTextureDescriptor` with react-native-webgpu's extensions;
/// `source` a `GPUTexture`'s id.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Descriptor {
    #[serde(default)]
    label: String,
    source: Id,
    #[serde(default)]
    color_space: Option<String>,
    #[serde(default)]
    rotation: Option<f64>,
    #[serde(default)]
    mirrored: bool,
    crop_origin: Option<Pair>,
    crop_size: Option<Pair>,
}

/// The 3x2 column-major affine matrix of `f` (`p = c0 * u + c1 * v + c2`).
fn affine(f: impl Fn(f32, f32) -> (f32, f32)) -> [f32; 6] {
    let (tx, ty) = f(0.0, 0.0);
    let (ux, uy) = f(1.0, 0.0);
    let (vx, vy) = f(0.0, 1.0);
    [ux - tx, uy - ty, vx - tx, vy - ty, tx, ty]
}

const SRGB: wgpu::ExternalTextureTransferFunction = wgpu::ExternalTextureTransferFunction {
    a: 1.055,
    b: 0.003_130_8,
    g: 2.4,
    k: 12.92,
};

/// The native half of `GPUExternalTexture`.
#[module]
pub struct WgpuExternalTexture;

#[module]
impl WgpuExternalTexture {
    /// `device.importExternalTexture(descriptor)` with a `GPUTexture`
    /// source: the external texture's id.
    pub fn import(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            if !entry
                .device
                .features()
                .contains(wgpu::Features::EXTERNAL_TEXTURE)
            {
                return Err(NativeError::unsupported(
                    "importExternalTexture on a device without wgpu's EXTERNAL_TEXTURE feature",
                ));
            }
            let desc: Descriptor = parse("GPUExternalTextureDescriptor", descriptor)?;
            let texture = registry::get::<TextureEntry>(desc.source)?.texture;
            let (tw, th) = (texture.width(), texture.height());
            let (ox, oy) = desc.crop_origin.as_ref().map_or(Ok((0, 0)), |p| p.get(0))?;
            let (cw, ch) = match &desc.crop_size {
                Some(p) => p.get(1)?,
                None => (tw.saturating_sub(ox), th.saturating_sub(oy)),
            };
            let rotation = desc.rotation.unwrap_or(0.0);
            // The plane's format and usage are wgpu's to validate (a
            // validation error); a crop outside the source throws, as
            // react-native-webgpu's does.
            if ox > tw || oy > th || cw > tw - ox || ch > th - oy || cw == 0 || ch == 0 {
                return Err(NativeError::operation(format!(
                    "importExternalTexture: the crop ({ox}, {oy}) + {cw} x {ch} is outside the {tw} x {th} source or empty"
                )));
            }
            if !matches!(rotation as u32, 0 | 90 | 180 | 270) || rotation.fract() != 0.0 {
                return Err(NativeError::type_error(format!(
                    "rotation {rotation} is not 0, 90, 180 or 270"
                )));
            }
            let rotation = rotation as u32;
            let (w, h) = if rotation % 180 == 90 {
                (ch, cw)
            } else {
                (cw, ch)
            };
            // Output (u, v) in [0, 1]² back to the source plane: undo the
            // rotation, then the mirror, then crop.
            let mirrored = desc.mirrored;
            let (ox, oy, cw_f, ch_f, tw_f, th_f) = (
                ox as f32, oy as f32, cw as f32, ch as f32, tw as f32, th as f32,
            );
            let to_plane = move |u: f32, v: f32| {
                let (s, t) = match rotation {
                    90 => (v, 1.0 - u),
                    180 => (1.0 - u, 1.0 - v),
                    270 => (1.0 - v, u),
                    _ => (u, v),
                };
                let s = if mirrored { 1.0 - s } else { s };
                ((ox + s * cw_f) / tw_f, (oy + t * ch_f) / th_f)
            };
            let sample_transform = affine(to_plane);
            let (wf, hf) = (w as f32, h as f32);
            let load_transform = affine(|x, y| {
                let (pu, pv) = to_plane((x + 0.5) / wf, (y + 0.5) / hf);
                (pu * tw_f - 0.5, pv * th_f - 0.5)
            });
            let p3 = match desc.color_space.as_deref() {
                None | Some("srgb") => false,
                Some("display-p3") => true,
                Some(other) => {
                    return Err(NativeError::type_error(format!(
                        "'{other}' is not a valid PredefinedColorSpace"
                    )));
                }
            };
            let (gamut, src_tf, dst_tf) = if p3 {
                // Linear sRGB to linear Display P3, column-major.
                (
                    [
                        0.822_462_1,
                        0.033_194_2,
                        0.017_082_7,
                        0.177_538,
                        0.966_805_8,
                        0.072_397_4,
                        0.0,
                        0.0,
                        0.910_519_9,
                    ],
                    SRGB,
                    SRGB,
                )
            } else {
                (
                    [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                    wgpu::ExternalTextureTransferFunction::default(),
                    wgpu::ExternalTextureTransferFunction::default(),
                )
            };
            let plane = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let external_texture = entry.device.create_external_texture(
                &wgpu::ExternalTextureDescriptor {
                    label: label(&desc.label),
                    width: w,
                    height: h,
                    format: wgpu::ExternalTextureFormat::Rgba,
                    yuv_conversion_matrix: [
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
                        1.0,
                    ],
                    gamut_conversion_matrix: gamut,
                    src_transfer_function: src_tf,
                    dst_transfer_function: dst_tf,
                    sample_transform,
                    load_transform,
                },
                &[&plane],
            );
            Ok(registry::insert(ExternalTextureEntry {
                external_texture,
                device,
            }))
        })
    }

    /// `externalTexture.destroy()` (react-native-webgpu's extension):
    /// releases the plane; idempotent. Sampling it afterwards is a
    /// validation error at submit.
    pub fn destroy(external_texture: u32) -> Result<(), String> {
        guard(|| {
            let entry = registry::get::<ExternalTextureEntry>(external_texture)?;
            entry.external_texture.destroy();
            Ok(())
        })
    }

    /// Whether `device` can import external textures (wgpu's
    /// `EXTERNAL_TEXTURE` feature).
    pub fn supported(device: u32) -> Result<bool, String> {
        guard(|| {
            Ok(DeviceEntry::get(device)?
                .device
                .features()
                .contains(wgpu::Features::EXTERNAL_TEXTURE))
        })
    }
}

/// The transforms, read back with `textureLoad` of a `texture_external`
/// in a compute shader (bind groups made by wgpu directly: area (a)'s do
/// not take external textures yet).
#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn device() -> Option<(Id, wgpu::Device, wgpu::Queue)> {
        let adapter = crate::instance::request_adapter(&Default::default())?;
        if !adapter
            .features()
            .contains(wgpu::Features::EXTERNAL_TEXTURE)
        {
            return None;
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::EXTERNAL_TEXTURE,
            ..Default::default()
        }))
        .ok()?;
        Some((
            DeviceEntry::register(device.clone(), queue.clone(), None),
            device,
            queue,
        ))
    }

    const RED: [u8; 4] = [255, 0, 0, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];

    /// The external texture `desc` makes of a 2 x 2 (red, green / blue,
    /// white) texture: its size and texels, row by row.
    /// An external texture's size and texels.
    type Loaded = ((u32, u32), Vec<[u8; 4]>);

    fn load(desc: serde_json::Value) -> Option<Loaded> {
        let (id, device, queue) = device()?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            texture.as_image_copy(),
            &[RED, GREEN, BLUE, WHITE].concat(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: None,
            },
            texture.size(),
        );
        let source = registry::insert(TextureEntry {
            texture,
            device: id,
            frame_of: None,
        });
        let mut desc = desc;
        desc["source"] = json!(source);
        let external = WgpuExternalTexture::import(id, desc).unwrap();
        let external = registry::get::<ExternalTextureEntry>(external)
            .unwrap()
            .external_texture;
        let out = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 4 * 16 + 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: out.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(
                "@group(0) @binding(0) var t: texture_external;
                 @group(0) @binding(1) var<storage, read_write> o: array<vec4u, 5>;
                 @compute @workgroup_size(1) fn main() {
                   let d = textureDimensions(t);
                   o[4] = vec4u(d, 0, 0);
                   for (var y = 0u; y < d.y; y++) {
                     for (var x = 0u; x < d.x; x++) {
                       o[y * d.x + x] = vec4u(round(textureLoad(t, vec2u(x, y)) * 255.0));
                     }
                   }
                 }"
                .into(),
            ),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::ExternalTexture(&external),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: out.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&out, 0, &read, 0, None);
        queue.submit([encoder.finish()]);
        read.slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let words: Vec<u32> = read
            .slice(..)
            .get_mapped_range()
            .chunks(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        let size = (words[16], words[17]);
        let texels = (0..(size.0 * size.1) as usize)
            .map(|i| [0, 1, 2, 3].map(|c| words[i * 4 + c] as u8))
            .collect();
        Some((size, texels))
    }

    #[test]
    fn crop_mirror_and_rotation_reach_the_shader() {
        let Some(identity) = load(json!({})) else {
            eprintln!("no EXTERNAL_TEXTURE on this adapter: skipped");
            return;
        };
        assert_eq!(identity, ((2, 2), vec![RED, GREEN, BLUE, WHITE]));
        assert_eq!(
            load(json!({ "mirrored": true })).unwrap(),
            ((2, 2), vec![GREEN, RED, WHITE, BLUE])
        );
        // Clockwise: the bottom-left texel comes to the top-left.
        assert_eq!(
            load(json!({ "rotation": 90 })).unwrap(),
            ((2, 2), vec![BLUE, RED, WHITE, GREEN])
        );
        assert_eq!(
            load(json!({ "rotation": 180 })).unwrap(),
            ((2, 2), vec![WHITE, BLUE, GREEN, RED])
        );
        assert_eq!(
            load(json!({ "cropOrigin": [1, 0], "cropSize": [1, 2] })).unwrap(),
            ((1, 2), vec![GREEN, WHITE])
        );
        assert_eq!(
            load(json!({ "cropOrigin": { "x": 0, "y": 1 }, "cropSize": { "width": 2, "height": 1 }, "rotation": 90 }))
                .unwrap(),
            ((1, 2), vec![BLUE, WHITE])
        );
    }
}
