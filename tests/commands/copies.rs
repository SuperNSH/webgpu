//! The encoder's copies and clears: round trips, and the alignment rules
//! that make an encoder invalid.

use nsh_webgpu::api::{
    command_encoder::WgpuCommandEncoder as Encoder, device::WgpuDevice, queue::WgpuQueue,
    texture::WgpuTexture,
};
use serde_json::json;

use crate::common::*;

fn finish_and_submit(device: u32, encoder: u32) -> Option<String> {
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    let commands = Encoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
    pop_error(device)
}

#[test]
fn copy_buffer_to_buffer_with_offsets_and_the_short_form() {
    let device = device();
    let src = buffer_with(device, usage::COPY_SRC, &u32s(&[1, 2, 3, 4]));
    let dst = buffer_with(device, usage::COPY_SRC, &[0; 16]);
    let encoder = Encoder::create(device, json!(null)).unwrap();
    Encoder::copy_buffer_to_buffer(encoder, src, 4.0, dst, 8.0, Some(8.0)).unwrap();
    assert_eq!(finish_and_submit(device, encoder), None);
    assert_eq!(read_back(device, dst, 16), u32s(&[0, 0, 2, 3]));
    // size omitted: to the source's end.
    let encoder = Encoder::create(device, json!(null)).unwrap();
    Encoder::copy_buffer_to_buffer(encoder, src, 0.0, dst, 0.0, None).unwrap();
    assert_eq!(finish_and_submit(device, encoder), None);
    assert_eq!(read_back(device, dst, 16), u32s(&[1, 2, 3, 4]));
}

#[test]
fn misaligned_or_out_of_range_buffer_copies_invalidate_the_encoder() {
    let device = device();
    let src = buffer_with(device, usage::COPY_SRC, &[0; 16]);
    let dst = buffer_with(device, usage::COPY_SRC, &[0; 16]);
    for (so, doff, size, why) in [
        (2.0, 0.0, 4.0, "multiples of 4"),
        (0.0, 0.0, 6.0, "multiples of 4"),
        (8.0, 0.0, 12.0, "do not fit"),
    ] {
        let encoder = Encoder::create(device, json!(null)).unwrap();
        Encoder::copy_buffer_to_buffer(encoder, src, so, dst, doff, Some(size)).unwrap();
        let error = finish_and_submit(device, encoder).expect(why);
        assert!(error.contains(why), "{error}");
    }
}

#[test]
fn clear_buffer_zeroes_a_range() {
    let device = device();
    let buffer = buffer_with(device, usage::COPY_SRC, &u32s(&[1, 2, 3, 4]));
    let encoder = Encoder::create(device, json!(null)).unwrap();
    Encoder::clear_buffer(encoder, buffer, Some(4.0), Some(8.0)).unwrap();
    assert_eq!(finish_and_submit(device, encoder), None);
    assert_eq!(read_back(device, buffer, 16), u32s(&[1, 0, 0, 4]));
    let encoder = Encoder::create(device, json!(null)).unwrap();
    Encoder::clear_buffer(encoder, buffer, Some(8.0), None).unwrap();
    assert_eq!(finish_and_submit(device, encoder), None);
    assert_eq!(read_back(device, buffer, 16), u32s(&[1, 0, 0, 0]));
    let encoder = Encoder::create(device, json!(null)).unwrap();
    Encoder::clear_buffer(encoder, buffer, Some(2.0), None).unwrap();
    assert!(
        finish_and_submit(device, encoder)
            .unwrap()
            .contains("multiples of 4")
    );
}

#[test]
fn buffer_to_texture_to_texture_to_buffer_round_trips() {
    let device = device();
    let texels: Vec<u8> = (0..256u32).map(|v| v as u8).collect();
    // 8 x 8 rgba8unorm needs 32-byte rows; copies need 256.
    let mut padded = vec![0u8; 256 * 8];
    for y in 0..8 {
        padded[y * 256..y * 256 + 32].copy_from_slice(&texels[y * 32..y * 32 + 32]);
    }
    let src = buffer_with(device, usage::COPY_SRC, &padded);
    let texture = |extra: u32| {
        WgpuTexture::create(
            device,
            json!({ "size": [8, 8], "format": "rgba8unorm", "usage": usage::T_COPY_DST | usage::T_COPY_SRC | extra }),
        )
        .unwrap()
    };
    let (a, b) = (texture(0), texture(0));
    let encoder = Encoder::create(device, json!(null)).unwrap();
    Encoder::copy_buffer_to_texture(
        encoder,
        json!({ "buffer": src, "bytesPerRow": 256 }),
        json!({ "texture": a }),
        json!({ "width": 8, "height": 8 }),
    )
    .unwrap();
    Encoder::copy_texture_to_texture(
        encoder,
        json!({ "texture": a, "origin": [2, 2] }),
        json!({ "texture": b, "origin": { "x": 0, "y": 0 } }),
        json!([6, 6]),
    )
    .unwrap();
    assert_eq!(finish_and_submit(device, encoder), None);
    let out = read_texture(device, b, 8, 8, 4);
    // b's (0, 0) is a's (2, 2).
    assert_eq!(&out[0..4], &texels[(2 * 8 + 2) * 4..(2 * 8 + 2) * 4 + 4]);
    assert_eq!(
        &out[(5 * 8 + 5) * 4..(5 * 8 + 5) * 4 + 4],
        &texels[(7 * 8 + 7) * 4..(7 * 8 + 7) * 4 + 4]
    );
}

#[test]
fn bytes_per_row_must_be_a_multiple_of_256() {
    let device = device();
    let src = buffer_with(device, usage::COPY_SRC, &[0; 512]);
    let texture = WgpuTexture::create(
        device,
        json!({ "size": [8, 2], "format": "rgba8unorm", "usage": usage::T_COPY_DST }),
    )
    .unwrap();
    let encoder = Encoder::create(device, json!(null)).unwrap();
    Encoder::copy_buffer_to_texture(
        encoder,
        json!({ "buffer": src, "bytesPerRow": 32 }),
        json!({ "texture": texture }),
        json!([8, 2]),
    )
    .unwrap();
    let error = finish_and_submit(device, encoder).unwrap();
    assert!(error.contains("bytesPerRow 32"), "{error}");
}
