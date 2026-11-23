//! `createImageBitmap` and `copyExternalImageToTexture`:
//! `ImageData.spec.ts`, `ImageBitmapAlpha.spec.ts` (every case: NSH has no
//! Blob, so the blob cases decode the same bytes), `ImageBitmapOptions.spec.ts`
//! (the reference clients' snapshots: nsh-webgpu implements the options),
//! `ImageBitmapOrigin.spec.ts`, and destination formats and color spaces.

use nsh_webgpu::{
    api::{
        device::WgpuDevice, image_bitmap::WgpuImageBitmap, queue::WgpuQueue, texture::WgpuTexture,
    },
    bytes::Bytes,
};
use serde_json::{Value, json};

use crate::common::*;

fn decode(name: &str, options: Value) -> Result<Value, String> {
    block(WgpuImageBitmap::decode(Bytes(asset(name)), options))
}

/// Copies `bitmap` (`w` x `h`) into a new rgba8unorm texture with
/// `source`/`dest` extras; the texture's RGBA rows.
fn copy_to_texture(
    device: u32,
    bitmap: &Value,
    source: Value,
    dest: Value,
    size: [u32; 2],
) -> Vec<u8> {
    let texture = WgpuTexture::create(
        device,
        json!({
            "size": size,
            "format": "rgba8unorm",
            "usage": usage::T_COPY_DST | usage::T_COPY_SRC | usage::T_RENDER_ATTACHMENT,
        }),
    )
    .unwrap();
    let mut source = source;
    source["bitmap"] = bitmap["id"].clone();
    let mut dest = dest;
    dest["texture"] = json!(texture);
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    WgpuQueue::copy_external_image_to_texture(device, source, dest, json!(size)).unwrap();
    assert_eq!(pop_error(device), None);
    read_texture(device, texture, size[0], size[1], 4)
}

fn flip_rows(rgba: &[u8], w: u32) -> Vec<u8> {
    rgba.chunks((w * 4) as usize)
        .rev()
        .flatten()
        .copied()
        .collect()
}

#[test]
fn image_data_decodes_like_the_reference() {
    // ImageData.spec.ts "createImageBitmap (1)/(2)": Di-3d.png decodes to
    // ref.png (straight alpha, as the reference's harness decodes it).
    let bitmap = decode("assets/Di-3d.png", json!({ "premultiplyAlpha": "none" })).unwrap();
    assert_eq!(
        (bitmap["width"].as_u64(), bitmap["height"].as_u64()),
        (Some(512), Some(512))
    );
    let pixels = WgpuImageBitmap::pixels(bitmap["id"].as_u64().unwrap() as u32).unwrap();
    check_image(&pixels, 512, 512, "ref.png");
    // close() lets the pixels go; closing twice does nothing.
    let id = bitmap["id"].as_u64().unwrap() as u32;
    WgpuImageBitmap::close(id);
    WgpuImageBitmap::close(id);
    assert!(WgpuImageBitmap::pixels(id).is_err());
}

#[test]
fn undecodable_bytes_are_an_invalid_state_error() {
    let e = block(WgpuImageBitmap::decode(
        Bytes(vec![1, 2, 3, 4]),
        json!(null),
    ))
    .unwrap_err();
    assert!(e.starts_with("InvalidStateError:"), "{e}");
}

fn alpha_snapshot(source: &str, destination: Option<bool>) -> &'static str {
    if destination == Some(true) {
        "image-bitmap-alpha-premultiplied.png"
    } else if source == "none" {
        "image-bitmap-alpha-straight.png"
    } else {
        "image-bitmap-alpha-roundtrip.png"
    }
}

#[test]
fn every_alpha_case_matches_its_snapshot() {
    // ImageBitmapAlpha.spec.ts: the 18 + 1 "blob" cases and the 4 buffer
    // cases (the same bytes here).
    let device = device();
    let mut cases = Vec::new();
    for source in ["none", "premultiply", "omitted"] {
        for destination in [None, Some(false), Some(true)] {
            for flip in [false, true] {
                cases.push((source, destination, flip));
            }
        }
    }
    cases.push(("default", Some(false), false));
    for source in ["none", "premultiply"] {
        for destination in [false, true] {
            cases.push((source, Some(destination), false));
        }
    }
    assert_eq!(cases.len(), 23);
    for (source, destination, flip) in cases {
        let options = if source == "omitted" {
            json!(null)
        } else {
            json!({ "premultiplyAlpha": source })
        };
        let bitmap = decode("assets/alpha-gradient.png", options).unwrap();
        let dest = match destination {
            None => json!({}),
            Some(p) => json!({ "premultipliedAlpha": p }),
        };
        let rgba = copy_to_texture(device, &bitmap, json!({ "flipY": flip }), dest, [32, 32]);
        let upright = if flip { flip_rows(&rgba, 32) } else { rgba };
        check_image(&upright, 32, 32, alpha_snapshot(source, destination));
    }
}

#[test]
fn alpha_conversions_are_bit_exact() {
    // ImageBitmapAlpha.spec.ts "preserves exact bytes": a 1 x 2 PNG.
    let device = device();
    let mut png = Vec::new();
    image::RgbaImage::from_raw(1, 2, vec![128, 128, 128, 128, 17, 34, 51, 64])
        .unwrap()
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    for (destination, expected) in [
        (false, [128, 128, 128, 128, 17, 34, 51, 64]),
        (true, [64, 64, 64, 128, 4, 9, 13, 64]),
    ] {
        let bitmap = block(WgpuImageBitmap::decode(
            Bytes(png.clone()),
            json!({ "premultiplyAlpha": "none" }),
        ))
        .unwrap();
        let rgba = copy_to_texture(
            device,
            &bitmap,
            json!({}),
            json!({ "premultipliedAlpha": destination }),
            [1, 2],
        );
        assert_eq!(rgba, expected);
    }
}

#[test]
fn options_match_the_reference_clients() {
    // ImageBitmapOptions.spec.ts against the snapshots Chrome wrote.
    let device = device();
    let cases: [(Value, &str); 6] = [
        (
            json!({ "crop": [8, 4, 16, 24] }),
            "image-bitmap-options-crop.png",
        ),
        (
            json!({ "resizeWidth": 16, "resizeHeight": 16 }),
            "image-bitmap-options-resize-down.png",
        ),
        (
            json!({ "resizeWidth": 64, "resizeHeight": 48 }),
            "image-bitmap-options-resize-up.png",
        ),
        (
            json!({ "crop": [8, 4, 16, 24], "resizeWidth": 32, "resizeHeight": 32 }),
            "image-bitmap-options-crop-resize.png",
        ),
        (
            json!({ "imageOrientation": "flipY" }),
            "image-bitmap-options-flip.png",
        ),
        (
            json!({ "imageOrientation": "from-image" }),
            "assets/opaque-gradient.png",
        ),
    ];
    for (options, snapshot) in cases {
        let bitmap = decode("assets/opaque-gradient.png", options).unwrap();
        let (w, h) = (
            bitmap["width"].as_u64().unwrap() as u32,
            bitmap["height"].as_u64().unwrap() as u32,
        );
        let rgba = copy_to_texture(device, &bitmap, json!({}), json!({}), [w, h]);
        check_image(&rgba, w, h, snapshot);
    }
    // colorSpaceConversion "none": the raw values, the asset itself.
    let bitmap = decode(
        "assets/p3-gradient.png",
        json!({ "colorSpaceConversion": "none" }),
    )
    .unwrap();
    let rgba = copy_to_texture(device, &bitmap, json!({}), json!({}), [32, 32]);
    check_image(&rgba, 32, 32, "assets/p3-gradient.png");
    // A zero crop size is a RangeError, a zero resize an InvalidStateError.
    let e = decode(
        "assets/opaque-gradient.png",
        json!({ "crop": [0, 0, 0, 4] }),
    )
    .unwrap_err();
    assert!(e.starts_with("RangeError:"), "{e}");
    let e = decode("assets/opaque-gradient.png", json!({ "resizeWidth": 0 })).unwrap_err();
    assert!(e.starts_with("InvalidStateError:"), "{e}");
}

#[test]
fn a_crop_outside_the_image_is_transparent_and_a_negative_size_extends_back() {
    let pixels = |options: Value| {
        let b = decode("assets/opaque-gradient.png", options).unwrap();
        (
            b["width"].as_u64().unwrap(),
            WgpuImageBitmap::pixels(b["id"].as_u64().unwrap() as u32).unwrap(),
        )
    };
    let (w, out) = pixels(json!({ "crop": [28, 0, 8, 1], "premultiplyAlpha": "none" }));
    assert_eq!(w, 8);
    assert_eq!(out[4 * 4 + 3], 0, "past the right edge: transparent");
    assert_eq!(out[3], 255, "inside: opaque");
    let (_, a) = pixels(json!({ "crop": [8, 0, -4, 1] }));
    let (_, b) = pixels(json!({ "crop": [4, 0, 4, 1] }));
    assert_eq!(a, b);
}

#[test]
fn the_source_origin_selects_the_region() {
    // ImageBitmapOrigin.spec.ts: the region copy equals the same region
    // of the full copy.
    let device = device();
    let cases = [
        ("assets/opaque-gradient.png", [8, 0], [16, 32], false, false),
        ("assets/opaque-gradient.png", [0, 8], [32, 16], false, false),
        ("assets/alpha-gradient.png", [8, 4], [16, 20], false, true),
        ("assets/opaque-gradient.png", [8, 4], [16, 20], true, false),
        ("assets/opaque-gradient.png", [0, 0], [16, 16], false, false),
    ];
    for (name, origin, size, flip, premultiplied) in cases {
        let bitmap = decode(
            name,
            json!({ "premultiplyAlpha": if premultiplied { "premultiply" } else { "none" } }),
        )
        .unwrap();
        let full = copy_to_texture(
            device,
            &bitmap,
            json!({}),
            json!({ "premultipliedAlpha": false }),
            [32, 32],
        );
        let region = copy_to_texture(
            device,
            &bitmap,
            json!({ "origin": origin, "flipY": flip }),
            json!({ "premultipliedAlpha": false }),
            size,
        );
        let mut expected = Vec::new();
        for row in 0..size[1] {
            let src_row = origin[1] + if flip { size[1] - 1 - row } else { row };
            let start = ((src_row * 32 + origin[0]) * 4) as usize;
            expected.extend_from_slice(&full[start..start + (size[0] * 4) as usize]);
        }
        assert_eq!(region, expected, "{name} {origin:?} {size:?} flip={flip}");
    }
}

#[test]
fn a_region_outside_the_bitmap_is_an_operation_error() {
    let device = device();
    let bitmap = decode("assets/opaque-gradient.png", json!(null)).unwrap();
    for (origin, size) in [([17, 0], [16, 32]), ([0, 17], [32, 16])] {
        let texture = WgpuTexture::create(
            device,
            json!({ "size": size, "format": "rgba8unorm", "usage": usage::T_COPY_DST | usage::T_RENDER_ATTACHMENT }),
        )
        .unwrap();
        let e = WgpuQueue::copy_external_image_to_texture(
            device,
            json!({ "bitmap": bitmap["id"], "origin": origin }),
            json!({ "texture": texture }),
            json!(size),
        )
        .unwrap_err();
        assert!(e.starts_with("OperationError:"), "{e}");
    }
}

#[test]
fn the_destination_needs_render_attachment_and_a_listed_format() {
    let device = device();
    let bitmap = decode("assets/opaque-gradient.png", json!(null)).unwrap();
    for (format, usage_bits, why) in [
        ("rgba8unorm", usage::T_COPY_DST, "RENDER_ATTACHMENT"),
        (
            "rgba8uint",
            usage::T_COPY_DST | usage::T_RENDER_ATTACHMENT,
            "format",
        ),
    ] {
        let texture = WgpuTexture::create(
            device,
            json!({ "size": [32, 32], "format": format, "usage": usage_bits }),
        )
        .unwrap();
        WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
        WgpuQueue::copy_external_image_to_texture(
            device,
            json!({ "bitmap": bitmap["id"] }),
            json!({ "texture": texture }),
            json!([32, 32]),
        )
        .unwrap();
        let error = pop_error(device).unwrap_or_default();
        assert!(error.contains(why), "{format}: {error}");
    }
}

#[test]
fn destination_formats_get_the_same_colors() {
    // The opaque gradient into bgra8unorm, rgba16float and rgb10a2unorm
    // reads back as the rgba8unorm copy does.
    let device = device();
    let bitmap = decode(
        "assets/opaque-gradient.png",
        json!({ "premultiplyAlpha": "none" }),
    )
    .unwrap();
    let reference = copy_to_texture(device, &bitmap, json!({}), json!({}), [32, 32]);
    for (format, bpp) in [
        ("bgra8unorm", 4),
        ("rgba16float", 8),
        ("rgb10a2unorm", 4),
        ("rgba32float", 16),
    ] {
        let texture = WgpuTexture::create(
            device,
            json!({ "size": [32, 32], "format": format, "usage": usage::T_COPY_DST | usage::T_COPY_SRC | usage::T_RENDER_ATTACHMENT }),
        )
        .unwrap();
        WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
        WgpuQueue::copy_external_image_to_texture(
            device,
            json!({ "bitmap": bitmap["id"] }),
            json!({ "texture": texture }),
            json!([32, 32]),
        )
        .unwrap();
        assert_eq!(pop_error(device), None, "{format}");
        let raw = read_texture(device, texture, 32, 32, bpp);
        let rgba: Vec<u8> = raw
            .chunks(bpp as usize)
            .flat_map(|t| -> [u8; 4] {
                match format {
                    "bgra8unorm" => [t[2], t[1], t[0], t[3]],
                    "rgba16float" => [0, 1, 2, 3].map(|c| {
                        let h = u16::from_le_bytes([t[c * 2], t[c * 2 + 1]]);
                        let (e, m) = (i32::from((h >> 10) & 0x1f), f32::from(h & 0x3ff));
                        let v = if e == 0 {
                            m / 1024.0 * 2f32.powi(-14)
                        } else {
                            2f32.powi(e - 15) * (1.0 + m / 1024.0)
                        };
                        (v * 255.0).round() as u8
                    }),
                    "rgba32float" => [0, 1, 2, 3].map(|c| {
                        (f32::from_le_bytes(t[c * 4..c * 4 + 4].try_into().unwrap()) * 255.0)
                            .round() as u8
                    }),
                    _ => {
                        let v = u32::from_le_bytes(t.try_into().unwrap());
                        let ten =
                            |s: u32| ((f64::from((v >> s) & 0x3ff) / 1023.0) * 255.0).round() as u8;
                        [
                            ten(0),
                            ten(10),
                            ten(20),
                            (((v >> 30) as f64 / 3.0) * 255.0).round() as u8,
                        ]
                    }
                }
            })
            .collect();
        let close = rgba
            .iter()
            .zip(&reference)
            .all(|(a, b)| a.abs_diff(*b) <= 1);
        assert!(close, "{format} differs from rgba8unorm");
    }
}

#[test]
fn a_display_p3_destination_converts_the_colors() {
    let device = device();
    let bitmap = block(WgpuImageBitmap::decode(
        Bytes({
            let mut png = Vec::new();
            image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 255, 255, 255, 255])
                .unwrap()
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .unwrap();
            png
        }),
        json!(null),
    ))
    .unwrap();
    let srgb = copy_to_texture(device, &bitmap, json!({}), json!({}), [2, 1]);
    let p3 = copy_to_texture(
        device,
        &bitmap,
        json!({}),
        json!({ "colorSpace": "display-p3" }),
        [2, 1],
    );
    assert_eq!(&srgb[..4], &[255, 0, 0, 255]);
    // sRGB red in P3 (about 234, 51, 35), white stays white.
    assert!(
        p3[0] < 245 && p3[0] > 225 && p3[1] > 40 && p3[2] > 25,
        "{:?}",
        &p3[..4]
    );
    assert_eq!(&p3[4..8], &[255, 255, 255, 255]);
}

#[test]
fn image_data_pixels_make_a_bitmap() {
    let device = device();
    let pixels = vec![255, 0, 0, 128, 0, 255, 0, 255];
    let made = WgpuImageBitmap::from_pixels(
        Bytes(pixels.clone()),
        2,
        1,
        json!({ "premultiplyAlpha": "none" }),
    )
    .unwrap();
    assert_eq!(made["premultiplied"], false);
    let rgba = copy_to_texture(device, &made, json!({}), json!({}), [2, 1]);
    assert_eq!(rgba, pixels);
    let e = WgpuImageBitmap::from_pixels(Bytes(vec![0; 7]), 2, 1, json!(null)).unwrap_err();
    assert!(e.starts_with("InvalidStateError:"), "{e}");
}
