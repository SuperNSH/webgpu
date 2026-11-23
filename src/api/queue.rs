//! `GPUQueue` (`device.queue`): `submit`, `writeBuffer`, `writeTexture`,
//! `onSubmittedWorkDone`, `copyExternalImageToTexture`.
//!
//! `copyExternalImageToTexture` takes an `ImageBitmap` (`image_bitmap.rs`;
//! TSX makes one of an `ImageData` first) and writes it with
//! `write_texture`, as react-native-webgpu does on the CPU: the source
//! region (`origin`, `flipY`) is staged, converted between straight and
//! premultiplied alpha with react-native-webgpu's integer rounding, to
//! Display P3 when the destination asks for it, and encoded in the
//! destination's format (the spec's list: `r8unorm` ... `rgba32float`).

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::{
        buffer::BufferEntry,
        command_buffer::CommandBufferEntry,
        command_encoder::enforce_u64,
        device::DeviceEntry,
        image_bitmap::{ImageBitmapEntry, convert_alpha},
        texture::TextureEntry,
    },
    bytes::Bytes,
    convert::{Extent3d, Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo, parse},
    error::{GpuErrorKind, NativeError, guard},
    pending, registry,
};

/// `GPUCopyExternalImageSourceInfo` with the bitmap as an id.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceInfo {
    bitmap: u32,
    #[serde(default)]
    origin: Origin3d,
    #[serde(default)]
    flip_y: bool,
}

/// `GPUCopyExternalImageDestInfo`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DestInfo {
    #[serde(flatten)]
    texture: TexelCopyTextureInfo,
    #[serde(default)]
    color_space: Option<String>,
    #[serde(default)]
    premultiplied_alpha: bool,
}

/// sRGB's transfer function, decoding.
fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB's transfer function, encoding (Display P3 shares it).
fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Straight sRGB RGBA8 pixels in Display P3 (same transfer function,
/// linear sRGB to linear P3 primaries).
fn srgb_to_display_p3(data: &mut [u8]) {
    const M: [[f32; 3]; 3] = [
        [0.822_462_1, 0.177_538, 0.0],
        [0.033_194_2, 0.966_805_8, 0.0],
        [0.017_082_7, 0.072_397_4, 0.910_519_9],
    ];
    for px in data.as_chunks_mut::<4>().0 {
        let lin = [0, 1, 2].map(|c| srgb_to_linear(f32::from(px[c]) / 255.0));
        for (c, row) in M.iter().enumerate() {
            let v = row[0] * lin[0] + row[1] * lin[1] + row[2] * lin[2];
            px[c] = (linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0).round() as u8;
        }
    }
}

/// An `f32` as IEEE half-precision bits (round to nearest even).
fn f16_bits(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mant = bits & 0x7f_ffff;
    if exp <= 0 {
        if exp < -10 {
            return sign;
        }
        let m = (mant | 0x80_0000) >> (1 - exp);
        let round = (m & 0x1fff) > 0x1000 || ((m & 0x1fff) == 0x1000 && (m & 0x2000) != 0);
        return sign | ((m >> 13) as u16 + u16::from(round));
    }
    if exp >= 31 {
        return sign | 0x7c00;
    }
    let half = sign | ((exp as u16) << 10) | ((mant >> 13) as u16);
    let rest = mant & 0x1fff;
    let round = rest > 0x1000 || (rest == 0x1000 && (half & 1) == 1);
    half + u16::from(round)
}

/// RGBA8 `rgba` (as the destination wants them: alpha and color space
/// done) in `format`'s texels; `None` for a format the spec does not allow.
fn encode(rgba: &[u8], format: wgpu::TextureFormat) -> Option<(Vec<u8>, u32)> {
    use wgpu::TextureFormat as F;
    let unit = |v: u8| f32::from(v) / 255.0;
    let pixels = rgba.as_chunks::<4>().0.iter();
    let (bytes, size): (Vec<u8>, u32) = match format {
        F::Rgba8Unorm | F::Rgba8UnormSrgb => (rgba.to_vec(), 4),
        F::Bgra8Unorm | F::Bgra8UnormSrgb => {
            (pixels.flat_map(|p| [p[2], p[1], p[0], p[3]]).collect(), 4)
        }
        F::R8Unorm => (pixels.map(|p| p[0]).collect(), 1),
        F::Rg8Unorm => (pixels.flat_map(|p| [p[0], p[1]]).collect(), 2),
        F::R16Float => (
            pixels
                .flat_map(|p| f16_bits(unit(p[0])).to_le_bytes())
                .collect(),
            2,
        ),
        F::Rg16Float => (
            pixels
                .flat_map(|p| [p[0], p[1]].map(|v| f16_bits(unit(v)).to_le_bytes()))
                .flatten()
                .collect(),
            4,
        ),
        F::Rgba16Float => (
            pixels
                .flat_map(|p| [p[0], p[1], p[2], p[3]].map(|v| f16_bits(unit(v)).to_le_bytes()))
                .flatten()
                .collect(),
            8,
        ),
        F::R32Float => (pixels.flat_map(|p| unit(p[0]).to_le_bytes()).collect(), 4),
        F::Rg32Float => (
            pixels
                .flat_map(|p| [p[0], p[1]].map(|v| unit(v).to_le_bytes()))
                .flatten()
                .collect(),
            8,
        ),
        F::Rgba32Float => (
            pixels
                .flat_map(|p| [p[0], p[1], p[2], p[3]].map(|v| unit(v).to_le_bytes()))
                .flatten()
                .collect(),
            16,
        ),
        F::Rgb10a2Unorm => (
            pixels
                .flat_map(|p| {
                    let ten = |v: u8| ((f32::from(v) / 255.0) * 1023.0).round() as u32;
                    let two = ((f32::from(p[3]) / 255.0) * 3.0).round() as u32;
                    (ten(p[0]) | (ten(p[1]) << 10) | (ten(p[2]) << 20) | (two << 30)).to_le_bytes()
                })
                .collect(),
            4,
        ),
        _ => return None,
    };
    Some((bytes, size))
}

/// The native half of `GPUQueue`. A queue is named by its device's id.
#[module]
pub struct WgpuQueue;

#[module]
impl WgpuQueue {
    /// `queue.submit(commandBuffers)`. A command buffer submitted before,
    /// invalid, or of another device makes the whole submit a validation
    /// error, and nothing runs.
    pub fn submit(device: u32, command_buffers: Vec<u32>) -> Result<(), String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let mut buffers = Vec::with_capacity(command_buffers.len());
            let mut invalid = None;
            for id in &command_buffers {
                // Taken either way: a submitted command buffer is spent.
                let taken = registry::with_mut::<CommandBufferEntry, _>(*id, |cb| {
                    (cb.device, cb.buffer.take())
                })
                .ok();
                match taken {
                    Some((d, Some(buffer))) if d == device => buffers.push(buffer),
                    Some((_, Some(_))) => {
                        invalid.get_or_insert(format!(
                            "command buffer {id} belongs to another device"
                        ));
                    }
                    _ => {
                        invalid.get_or_insert(format!(
                            "command buffer {id} is invalid or was submitted before"
                        ));
                    }
                }
            }
            if let Some(message) = invalid {
                entry.report(GpuErrorKind::Validation, format!("submit: {message}"));
                return Ok(());
            }
            entry.queue.submit(buffers);
            Ok(())
        })
    }

    /// `queue.writeBuffer(buffer, bufferOffset, data)` with the bytes TSX
    /// cut from `data` (its `dataOffset` and `size`, in elements, checked
    /// there: an `OperationError`). The buffer range is validated here.
    pub fn write_buffer(
        device: u32,
        buffer: u32,
        buffer_offset: f64,
        data: Bytes,
    ) -> Result<(), String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let offset = enforce_u64("bufferOffset", buffer_offset)?;
            let target = registry::get::<BufferEntry>(buffer)?;
            let size = data.0.len() as u64;
            // wgpu panics on these; the spec makes them validation errors.
            if !offset.is_multiple_of(4)
                || !size.is_multiple_of(4)
                || offset
                    .checked_add(size)
                    .is_none_or(|end| end > target.buffer.size())
            {
                entry.report(
                    GpuErrorKind::Validation,
                    format!(
                        "writeBuffer: [{offset}, {}) must start and end at multiples of 4 inside {} bytes",
                        offset.saturating_add(size),
                        target.buffer.size()
                    ),
                );
                return Ok(());
            }
            if !target.buffer.usage().contains(wgpu::BufferUsages::COPY_DST) {
                entry.report(
                    GpuErrorKind::Validation,
                    "writeBuffer: the buffer's usage has no COPY_DST",
                );
                return Ok(());
            }
            if size > 0 {
                entry.queue.write_buffer(&target.buffer, offset, &data.0);
            }
            Ok(())
        })
    }

    /// `queue.onSubmittedWorkDone()`: a ticket to `Wgpu.wait` on.
    pub fn on_submitted_work_done(device: u32) -> Result<u32, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            // Covers the writes not submitted yet (`writeBuffer`), which wgpu
            // holds until a submit.
            entry.queue.submit([]);
            let (ticket, slot) = pending::start(&entry.device);
            entry
                .queue
                .on_submitted_work_done(move || pending::fill(&slot, Ok(())));
            Ok(ticket)
        })
    }

    /// `queue.writeTexture(destination, data, dataLayout, size)`: `data` is
    /// the whole view (`dataLayout.offset` counts in bytes from its start).
    pub fn write_texture(
        device: u32,
        destination: JsonValue,
        data: Bytes,
        data_layout: JsonValue,
        size: JsonValue,
    ) -> Result<(), String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let destination: TexelCopyTextureInfo = parse("GPUTexelCopyTextureInfo", destination)?;
            let layout: TexelCopyBufferLayout = parse("GPUTexelCopyBufferLayout", data_layout)?;
            let size = parse::<Extent3d>("GPUExtent3D", size)?.to_wgpu()?;
            let texture = registry::get::<TextureEntry>(destination.texture)?.texture;
            if layout.offset > data.0.len() as u64 {
                entry.report(
                    GpuErrorKind::Validation,
                    format!(
                        "writeTexture: dataLayout.offset {} is past the data's {} bytes",
                        layout.offset,
                        data.0.len()
                    ),
                );
                return Ok(());
            }
            entry.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: destination.mip_level,
                    origin: destination.origin.to_wgpu()?,
                    aspect: destination.aspect,
                },
                &data.0,
                layout.to_wgpu(),
                size,
            );
            Ok(())
        })
    }

    /// `queue.copyExternalImageToTexture(source, destination, copySize)`
    /// with `source.source` an `ImageBitmap`'s id (`source.bitmap`). A
    /// region outside the bitmap is an `OperationError`; a destination
    /// without `RENDER_ATTACHMENT | COPY_DST`, multisampled, or of a format
    /// the spec does not list is a validation error.
    pub fn copy_external_image_to_texture(
        device: u32,
        source: JsonValue,
        destination: JsonValue,
        copy_size: JsonValue,
    ) -> Result<(), String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let source: SourceInfo = parse("GPUCopyExternalImageSourceInfo", source)?;
            let destination: DestInfo = parse("GPUCopyExternalImageDestInfo", destination)?;
            let size = parse::<Extent3d>("GPUExtent3D", copy_size)?.to_wgpu()?;
            let origin = source.origin.to_wgpu()?;
            if origin.z != 0 {
                return Err(NativeError::type_error("source.origin is a GPUOrigin2D"));
            }
            let bitmap = registry::get::<ImageBitmapEntry>(source.bitmap)?;
            if u64::from(origin.x) + u64::from(size.width) > u64::from(bitmap.width)
                || u64::from(origin.y) + u64::from(size.height) > u64::from(bitmap.height)
                || size.depth_or_array_layers > 1
            {
                return Err(NativeError::operation(format!(
                    "copyExternalImageToTexture: the source copy region ({}, {}) + {} x {} x {} is outside the {} x {} ImageBitmap",
                    origin.x,
                    origin.y,
                    size.width,
                    size.height,
                    size.depth_or_array_layers,
                    bitmap.width,
                    bitmap.height
                )));
            }
            let texture = registry::get::<TextureEntry>(destination.texture.texture)?.texture;
            let needed = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST;
            let problem = if !texture.usage().contains(needed) {
                Some(
                    "the destination texture's usage must include RENDER_ATTACHMENT and COPY_DST"
                        .to_owned(),
                )
            } else if texture.sample_count() != 1 {
                Some("the destination texture must not be multisampled".to_owned())
            } else if !matches!(
                destination.color_space.as_deref(),
                None | Some("srgb") | Some("display-p3")
            ) {
                return Err(NativeError::type_error(format!(
                    "'{}' is not a valid PredefinedColorSpace",
                    destination.color_space.unwrap_or_default()
                )));
            } else {
                None
            };
            if let Some(problem) = problem {
                entry.report(
                    GpuErrorKind::Validation,
                    format!("copyExternalImageToTexture: {problem}"),
                );
                return Ok(());
            }
            // Stage the region, flipped if asked.
            let (w, h) = (size.width as usize, size.height as usize);
            let src_row = bitmap.width as usize * 4;
            let mut staged = vec![0u8; w * h * 4];
            for row in 0..h {
                let src_y = origin.y as usize + if source.flip_y { h - 1 - row } else { row };
                let start = src_y * src_row + origin.x as usize * 4;
                staged[row * w * 4..(row + 1) * w * 4]
                    .copy_from_slice(&bitmap.pixels[start..start + w * 4]);
            }
            if destination.color_space.as_deref() == Some("display-p3") {
                convert_alpha(&mut staged, bitmap.premultiplied, false);
                srgb_to_display_p3(&mut staged);
                convert_alpha(&mut staged, false, destination.premultiplied_alpha);
            } else {
                convert_alpha(
                    &mut staged,
                    bitmap.premultiplied,
                    destination.premultiplied_alpha,
                );
            }
            let Some((texels, texel_size)) = encode(&staged, texture.format()) else {
                entry.report(
                    GpuErrorKind::Validation,
                    format!(
                        "copyExternalImageToTexture: {:?} is not a format it can copy into",
                        texture.format()
                    ),
                );
                return Ok(());
            };
            if w == 0 || h == 0 {
                return Ok(());
            }
            entry.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: destination.texture.mip_level,
                    origin: destination.texture.origin.to_wgpu()?,
                    aspect: destination.texture.aspect,
                },
                &texels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size.width * texel_size),
                    rows_per_image: Some(size.height),
                },
                size,
            );
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_round_trip_the_values_a_byte_holds() {
        assert_eq!(f16_bits(0.0), 0);
        assert_eq!(f16_bits(1.0), 0x3c00);
        assert_eq!(f16_bits(0.5), 0x3800);
        assert_eq!(f16_bits(-2.0), 0xc000);
        // 1/255 is subnormal-free in half; check it decodes back closely.
        let h = f16_bits(1.0 / 255.0);
        let exp = ((h >> 10) & 0x1f) as i32;
        let mant = f32::from(h & 0x3ff);
        let back = 2f32.powi(exp - 15) * (1.0 + mant / 1024.0);
        assert!((back - 1.0 / 255.0).abs() < 1e-5, "{back}");
    }

    #[test]
    fn srgb_white_and_black_stay_in_display_p3() {
        let mut px = [255, 255, 255, 255, 0, 0, 0, 255, 255, 0, 0, 255];
        srgb_to_display_p3(&mut px);
        assert_eq!(&px[..8], &[255, 255, 255, 255, 0, 0, 0, 255]);
        // sRGB red is inside P3: less saturated there.
        assert!(px[8] < 255 && px[9] > 0, "{px:?}");
    }
}
