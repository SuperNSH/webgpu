//! `GPUQueue`: `ArrayBuffer.spec.ts`, `ArrayBufferBounds.spec.ts` (the
//! byte cases; element units and look-alike objects are TSX's, in
//! `examples/headless/commands.test.tsx`), `Texture.spec.ts`'s
//! `writeTexture` cases, `onSubmittedWorkDone`.

use nsh_webgpu::{
    api::{
        buffer::WgpuBuffer, command_encoder::WgpuCommandEncoder, device::WgpuDevice,
        queue::WgpuQueue, texture::WgpuTexture,
    },
    bytes::Bytes,
};
use serde_json::json;

use crate::common::*;

#[test]
fn array_buffer_round_trips_through_write_copy_and_map() {
    // ArrayBuffer.spec.ts (2) and (3): with and without onSubmittedWorkDone.
    for wait_done in [true, false] {
        let device = device();
        let data = f32s(&[1.0, 2.0, 3.0, 4.0]);
        let source = WgpuBuffer::create(
            device,
            json!({ "size": 16, "usage": usage::COPY_SRC | usage::COPY_DST }),
        )
        .unwrap();
        let read = WgpuBuffer::create(
            device,
            json!({ "size": 16, "usage": usage::COPY_DST | usage::MAP_READ }),
        )
        .unwrap();
        WgpuQueue::write_buffer(device, source, 0.0, Bytes(data.clone())).unwrap();
        let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
        WgpuCommandEncoder::copy_buffer_to_buffer(encoder, source, 0.0, read, 0.0, Some(16.0))
            .unwrap();
        let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
        WgpuQueue::submit(device, vec![commands]).unwrap();
        if wait_done {
            wait(WgpuQueue::on_submitted_work_done(device).unwrap()).unwrap();
        }
        assert_eq!(read_buffer(read), data);
    }
}

#[test]
fn write_buffer_checks_its_range_on_the_device() {
    let device = device();
    let buffer = buffer_with(device, usage::COPY_SRC, &[0; 8]);
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    WgpuQueue::write_buffer(device, buffer, 4.0, Bytes(vec![0; 8])).unwrap();
    assert!(pop_error(device).unwrap().contains("inside 8 bytes"));
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    WgpuQueue::write_buffer(device, buffer, 2.0, Bytes(vec![0; 4])).unwrap();
    assert!(pop_error(device).is_some());
    let e = WgpuQueue::write_buffer(device, buffer, -4.0, Bytes(vec![0; 4])).unwrap_err();
    assert!(e.starts_with("TypeError:"), "{e}");
    let no_copy_dst =
        WgpuBuffer::create(device, json!({ "size": 8, "usage": usage::COPY_SRC })).unwrap();
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    WgpuQueue::write_buffer(device, no_copy_dst, 0.0, Bytes(vec![0; 4])).unwrap();
    assert!(pop_error(device).unwrap().contains("COPY_DST"));
}

#[test]
fn write_texture_respects_the_layout_offset() {
    // Texture.spec.ts "writeTexture respects TypedArray byteOffset (#328)":
    // TSX hands over the view's bytes; here the layer starts at offset 256
    // of the data instead.
    let device = device();
    let mut data = Vec::new();
    for _ in 0..64 {
        data.extend([255, 0, 0, 255]);
    }
    for _ in 0..64 {
        data.extend([0, 255, 0, 255]);
    }
    let texture = WgpuTexture::create(
        device,
        json!({ "size": [64, 1], "format": "rgba8unorm", "usage": usage::T_COPY_SRC | usage::T_COPY_DST }),
    )
    .unwrap();
    WgpuQueue::write_texture(
        device,
        json!({ "texture": texture }),
        Bytes(data[256..].to_vec()),
        json!({ "bytesPerRow": 256 }),
        json!({ "width": 64, "height": 1 }),
    )
    .unwrap();
    assert_eq!(
        &read_texture(device, texture, 64, 1, 4)[..4],
        &[0, 255, 0, 255]
    );
    WgpuQueue::write_texture(
        device,
        json!({ "texture": texture }),
        Bytes(data.clone()),
        json!({ "offset": 256, "bytesPerRow": 256 }),
        json!([64, 1]),
    )
    .unwrap();
    assert_eq!(
        &read_texture(device, texture, 64, 1, 4)[..4],
        &[0, 255, 0, 255]
    );
    // writeTexture needs no 256-byte rows (unlike buffer copies).
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    WgpuQueue::write_texture(
        device,
        json!({ "texture": texture, "origin": [1, 0] }),
        Bytes(vec![9, 9, 9, 9, 8, 8, 8, 8]),
        json!({ "bytesPerRow": 8 }),
        json!([2, 1]),
    )
    .unwrap();
    assert_eq!(pop_error(device), None);
    assert_eq!(
        &read_texture(device, texture, 64, 1, 4)[4..12],
        &[9, 9, 9, 9, 8, 8, 8, 8]
    );
    // Too little data is a validation error, not a panic.
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    WgpuQueue::write_texture(
        device,
        json!({ "texture": texture }),
        Bytes(vec![0; 16]),
        json!({ "bytesPerRow": 256 }),
        json!([64, 1]),
    )
    .unwrap();
    assert!(pop_error(device).is_some());
}

#[test]
fn on_submitted_work_done_settles_after_the_work() {
    let device = device();
    let buffer = buffer_with(device, usage::COPY_SRC, &u32s(&[5, 6]));
    let ticket = WgpuQueue::on_submitted_work_done(device).unwrap();
    wait(ticket).unwrap();
    assert_eq!(read_back(device, buffer, 8), u32s(&[5, 6]));
}
