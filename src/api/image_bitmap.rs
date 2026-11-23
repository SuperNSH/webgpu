//! `createImageBitmap`: decoded images for `copyExternalImageToTexture`.
//!
//! react-native-webgpu decodes encoded bytes (an `ArrayBuffer` or a view;
//! a `Blob` on the web) with the platform's decoder into RGBA8 and keeps
//! them premultiplied unless `premultiplyAlpha: "none"`. Here the `image`
//! crate decodes PNG, JPEG and WebP, and the spec's other options are
//! applied too (react-native-webgpu ignores them; its reference clients
//! implement them): the crop rectangle, `resizeWidth`/`resizeHeight`
//! (bilinear, `"pixelated"` nearest), `imageOrientation: "flipY"`, in the
//! spec's order (crop, resize, orientation, alpha). `colorSpaceConversion`
//! is accepted and ignored (no ICC profiles: the bytes as encoded).
//!
//! An `ImageData` (straight RGBA8 from TSX) makes a bitmap the same way.

use std::sync::Arc;

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;
use serde_json::json;

#[cfg(target_arch = "wasm32")]
use crate::api::gpu::blocking;
use crate::{
    bytes::Bytes,
    convert::parse,
    error::{ErrorName, NativeError, guard},
    registry::{self, Id},
};

/// An `ImageBitmap` in the registry: RGBA8 rows, top to bottom.
#[derive(Clone)]
pub struct ImageBitmapEntry {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub pixels: Arc<Vec<u8>>,
    /// Whether the color channels are premultiplied by alpha.
    pub premultiplied: bool,
}

/// Converts RGBA8 `data` between straight and premultiplied alpha in
/// place, with react-native-webgpu's integer rounding
/// (`ImageBitmap.h` `convertAlpha`), so results are bit-exact with it.
pub fn convert_alpha(data: &mut [u8], source_premultiplied: bool, destination_premultiplied: bool) {
    if source_premultiplied == destination_premultiplied {
        return;
    }
    for px in data.as_chunks_mut::<4>().0 {
        let alpha = u32::from(px[3]);
        for c in &mut px[..3] {
            let value = u32::from(*c);
            *c = if destination_premultiplied {
                ((value * alpha + 127) / 255) as u8
            } else {
                (value * 255 + (alpha >> 1))
                    .checked_div(alpha)
                    .map_or(0, |v| v.min(255) as u8)
            };
        }
    }
}

/// `ImageBitmapOptions`, plus the crop rectangle of the 6-argument form.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Options {
    /// `[sx, sy, sw, sh]`.
    crop: Option<[f64; 4]>,
    #[serde(default)]
    premultiply_alpha: Option<String>,
    #[serde(default)]
    image_orientation: Option<String>,
    resize_width: Option<f64>,
    resize_height: Option<f64>,
    #[serde(default)]
    resize_quality: Option<String>,
    /// Accepted, ignored (no color management).
    #[serde(default)]
    #[allow(dead_code)]
    color_space_conversion: Option<String>,
}

/// An RGBA8 image, straight alpha.
struct Image {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

fn invalid_state(message: impl Into<String>) -> NativeError {
    NativeError::new(ErrorName::InvalidStateError, message)
}

fn range_error(message: impl Into<String>) -> NativeError {
    NativeError::new(ErrorName::RangeError, message)
}

/// The crop rectangle as the spec reads it: a negative width or height
/// extends left or up; pixels outside the image are transparent black.
fn crop(image: &Image, [sx, sy, sw, sh]: [f64; 4]) -> Result<Image, NativeError> {
    if sw == 0.0 || sh == 0.0 {
        return Err(range_error(
            "createImageBitmap: the crop width and height must not be 0",
        ));
    }
    let (x0, w) = if sw < 0.0 { (sx + sw, -sw) } else { (sx, sw) };
    let (y0, h) = if sh < 0.0 { (sy + sh, -sh) } else { (sy, sh) };
    let (x0, y0, w, h) = (
        x0.trunc() as i64,
        y0.trunc() as i64,
        w.trunc() as u32,
        h.trunc() as u32,
    );
    let mut data = vec![0u8; w as usize * h as usize * 4];
    for y in 0..h as i64 {
        let src_y = y0 + y;
        if src_y < 0 || src_y >= i64::from(image.height) {
            continue;
        }
        for x in 0..w as i64 {
            let src_x = x0 + x;
            if src_x < 0 || src_x >= i64::from(image.width) {
                continue;
            }
            let s = ((src_y as usize * image.width as usize) + src_x as usize) * 4;
            let d = ((y as usize * w as usize) + x as usize) * 4;
            data[d..d + 4].copy_from_slice(&image.data[s..s + 4]);
        }
    }
    Ok(Image {
        width: w,
        height: h,
        data,
    })
}

/// Bilinear resampling with edge clamp (the reference node client's
/// `resize`), or nearest for `"pixelated"`.
fn resize(image: &Image, width: u32, height: u32, nearest: bool) -> Image {
    let (sw, sh) = (image.width as usize, image.height as usize);
    let mut data = vec![0u8; width as usize * height as usize * 4];
    if sw == 0 || sh == 0 {
        return Image {
            width,
            height,
            data,
        };
    }
    let at = |x: usize, y: usize, c: usize| f64::from(image.data[(y * sw + x) * 4 + c]);
    for y in 0..height as usize {
        let src_y =
            (((y as f64 + 0.5) * sh as f64) / f64::from(height) - 0.5).clamp(0.0, (sh - 1) as f64);
        for x in 0..width as usize {
            let src_x = (((x as f64 + 0.5) * sw as f64) / f64::from(width) - 0.5)
                .clamp(0.0, (sw - 1) as f64);
            let d = (y * width as usize + x) * 4;
            if nearest {
                let (nx, ny) = (src_x.round() as usize, src_y.round() as usize);
                for c in 0..4 {
                    data[d + c] = at(nx, ny, c) as u8;
                }
                continue;
            }
            let (x0, y0) = (src_x.floor() as usize, src_y.floor() as usize);
            let (x1, y1) = ((x0 + 1).min(sw - 1), (y0 + 1).min(sh - 1));
            let (fx, fy) = (src_x - x0 as f64, src_y - y0 as f64);
            for c in 0..4 {
                let top = at(x0, y0, c) * (1.0 - fx) + at(x1, y0, c) * fx;
                let bottom = at(x0, y1, c) * (1.0 - fx) + at(x1, y1, c) * fx;
                data[d + c] = (top * (1.0 - fy) + bottom * fy).round() as u8;
            }
        }
    }
    Image {
        width,
        height,
        data,
    }
}

fn flip_vertically(image: &mut Image) {
    let row = image.width as usize * 4;
    let h = image.height as usize;
    for y in 0..h / 2 {
        let (top, bottom) = image.data.split_at_mut((h - 1 - y) * row);
        top[y * row..(y + 1) * row].swap_with_slice(&mut bottom[..row]);
    }
}

/// `[EnforceRange] unsigned long` for a resize dimension.
fn dimension(name: &str, value: f64) -> Result<u32, NativeError> {
    if !value.is_finite() || value.trunc() < 0.0 || value.trunc() > f64::from(u32::MAX) {
        return Err(NativeError::type_error(format!(
            "{name} ({value}) is not an unsigned long"
        )));
    }
    Ok(value.trunc() as u32)
}

/// Applies `options` to straight `image` in the spec's order; the bitmap.
fn finish(mut image: Image, options: &Options) -> Result<ImageBitmapEntry, NativeError> {
    if let Some(rect) = options.crop {
        image = crop(&image, rect)?;
    }
    let rw = options
        .resize_width
        .map(|v| dimension("resizeWidth", v))
        .transpose()?;
    let rh = options
        .resize_height
        .map(|v| dimension("resizeHeight", v))
        .transpose()?;
    if rw == Some(0) || rh == Some(0) {
        return Err(invalid_state(
            "createImageBitmap: resizeWidth and resizeHeight must not be 0",
        ));
    }
    let scaled = |a: u32, num: u32, den: u32| -> u32 {
        if den == 0 {
            0
        } else {
            ((f64::from(a) * f64::from(num)) / f64::from(den)).ceil() as u32
        }
    };
    let target = match (rw, rh) {
        (None, None) => None,
        (Some(w), Some(h)) => Some((w, h)),
        (Some(w), None) => Some((w, scaled(image.height, w, image.width))),
        (None, Some(h)) => Some((scaled(image.width, h, image.height), h)),
    };
    if let Some((w, h)) = target
        && (w, h) != (image.width, image.height)
    {
        let nearest = options.resize_quality.as_deref() == Some("pixelated");
        image = resize(&image, w, h, nearest);
    }
    if options.image_orientation.as_deref() == Some("flipY") {
        flip_vertically(&mut image);
    }
    let premultiplied = options.premultiply_alpha.as_deref() != Some("none");
    convert_alpha(&mut image.data, false, premultiplied);
    Ok(ImageBitmapEntry {
        width: image.width,
        height: image.height,
        pixels: Arc::new(image.data),
        premultiplied,
    })
}

/// Decodes encoded image `bytes` (PNG, JPEG, WebP) with `options`.
pub fn decode_bitmap(bytes: &[u8], options: JsonValue) -> Result<ImageBitmapEntry, NativeError> {
    let options: Options = parse("ImageBitmapOptions", options)?;
    let decoded = image::load_from_memory(bytes)
        .map_err(|e| {
            invalid_state(format!(
                "createImageBitmap: the source image could not be decoded ({e})"
            ))
        })?
        .to_rgba8();
    let (width, height) = decoded.dimensions();
    finish(
        Image {
            width,
            height,
            data: decoded.into_raw(),
        },
        &options,
    )
}

fn described(id: Id, entry: &ImageBitmapEntry) -> JsonValue {
    json!({ "id": id, "width": entry.width, "height": entry.height, "premultiplied": entry.premultiplied })
}

/// The native half of `ImageBitmap`.
#[module]
pub struct WgpuImageBitmap;

#[module]
impl WgpuImageBitmap {
    /// `createImageBitmap(bytes, options)`: decodes off the JS thread;
    /// `{ id, width, height, premultiplied }`. Bytes that do not decode
    /// reject with an `InvalidStateError`.
    pub async fn decode(bytes: Bytes, options: JsonValue) -> Result<JsonValue, String> {
        let entry = blocking(move || decode_bitmap(&bytes.0, options))
            .await
            .map_err(|e| NativeError::operation(format!("createImageBitmap: {e}")))??;
        let id = registry::insert(entry.clone());
        Ok(described(id, &entry))
    }

    /// `createImageBitmap(imageData, options)`: straight RGBA8 `pixels`,
    /// `width` × `height`; `{ id, width, height, premultiplied }`.
    pub fn from_pixels(
        pixels: Bytes,
        width: u32,
        height: u32,
        options: JsonValue,
    ) -> Result<JsonValue, String> {
        guard(|| {
            let options: Options = parse("ImageBitmapOptions", options)?;
            if pixels.0.len() as u64 != u64::from(width) * u64::from(height) * 4 {
                return Err(invalid_state(format!(
                    "createImageBitmap: {} bytes are not {width} x {height} RGBA pixels",
                    pixels.0.len()
                )));
            }
            let entry = finish(
                Image {
                    width,
                    height,
                    data: pixels.0,
                },
                &options,
            )?;
            let id = registry::insert(entry.clone());
            Ok(described(id, &entry))
        })
    }

    /// The bitmap's pixels, RGBA8 rows (premultiplied when it is).
    pub fn pixels(bitmap: u32) -> Result<Vec<u8>, String> {
        guard(|| Ok(registry::get::<ImageBitmapEntry>(bitmap)?.pixels.to_vec()))
    }

    /// `bitmap.close()`: its pixels go.
    pub fn close(bitmap: u32) {
        if registry::get::<ImageBitmapEntry>(bitmap).is_ok() {
            registry::release(bitmap);
        }
    }
}
