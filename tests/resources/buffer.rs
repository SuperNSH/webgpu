//! `GPUBuffer`: the reference's Buffer.spec (the parts the native side
//! answers) and the spec's mapping rules.

use nsh_webgpu::{
    api::{buffer::WgpuBuffer, device::WgpuDevice, queue::WgpuQueue},
    bytes::Bytes,
};
use serde_json::json;

use crate::common::{assert_throws, check_image, device, validation_error, wait};

const MAP_READ: u32 = 0x1;
const MAP_WRITE: u32 = 0x2;
const COPY_SRC: u32 = 0x4;
const COPY_DST: u32 = 0x8;
const VERTEX: u32 = 0x20;

#[test]
fn map_async_rejects_what_the_spec_rejects_with_a_validation_error() {
    let device = device();
    let readable =
        WgpuBuffer::create(device, json!({ "size": 64, "usage": MAP_READ | COPY_DST })).unwrap();
    let vertex = WgpuBuffer::create(device, json!({ "size": 64, "usage": VERTEX })).unwrap();
    for (buffer, mode, offset, size, why) in [
        (vertex, MAP_READ, None, None, "usage without MAP_READ"),
        (
            readable,
            MAP_WRITE,
            None,
            None,
            "WRITE on a MAP_READ buffer",
        ),
        (readable, MAP_READ | MAP_WRITE, None, None, "both modes"),
        (readable, 0, None, None, "no mode"),
        (
            readable,
            MAP_READ,
            Some(4.0),
            None,
            "offset not a multiple of 8",
        ),
        (
            readable,
            MAP_READ,
            Some(0.0),
            Some(6.0),
            "size not a multiple of 4",
        ),
        (readable, MAP_READ, Some(32.0), Some(64.0), "past the end"),
    ] {
        let (ticket, error) = validation_error(device, || {
            WgpuBuffer::map_async(buffer, mode, offset, size).unwrap()
        });
        assert!(error.is_some(), "{why}: no validation error");
        assert_throws(wait(ticket), "OperationError");
        assert_eq!(WgpuBuffer::map_state(buffer).unwrap(), "unmapped", "{why}");
    }
    // Mapping a mapped buffer is rejected too.
    wait(WgpuBuffer::map_async(readable, MAP_READ, None, None).unwrap()).unwrap();
    assert_throws(
        wait(WgpuBuffer::map_async(readable, MAP_READ, None, None).unwrap()),
        "OperationError",
    );
    assert_eq!(WgpuBuffer::map_state(readable).unwrap(), "mapped");
}

#[test]
fn get_mapped_range_checks_the_mapping_its_alignment_and_bounds() {
    let device = device();
    let buffer =
        WgpuBuffer::create(device, json!({ "size": 64, "usage": MAP_READ | COPY_DST })).unwrap();
    assert_throws(
        WgpuBuffer::get_mapped_range(buffer, None, None),
        "OperationError",
    );
    wait(WgpuBuffer::map_async(buffer, MAP_READ, Some(16.0), Some(32.0)).unwrap()).unwrap();
    assert_eq!(
        WgpuBuffer::get_mapped_range(buffer, Some(16.0), None)
            .unwrap()
            .len(),
        32
    );
    assert_eq!(
        WgpuBuffer::get_mapped_range(buffer, Some(24.0), Some(8.0))
            .unwrap()
            .len(),
        8
    );
    for (offset, size) in [(0.0, 8.0), (40.0, 16.0), (20.0, 4.0), (16.0, 6.0)] {
        assert_throws(
            WgpuBuffer::get_mapped_range(buffer, Some(offset), Some(size)),
            "OperationError",
        );
    }
    // A read mapping gives nothing back on unmap.
    assert_throws(
        WgpuBuffer::write_mapped(buffer, 16.0, Bytes(vec![1; 8])),
        "OperationError",
    );
}

#[test]
fn map_write_reaches_the_gpu_and_destroy_while_pending_aborts() {
    let device = device();
    let source =
        WgpuBuffer::create(device, json!({ "size": 16, "usage": MAP_WRITE | COPY_SRC })).unwrap();
    wait(WgpuBuffer::map_async(source, MAP_WRITE, None, None).unwrap()).unwrap();
    let floats: Vec<u8> = [5.0f32, 6.0, 7.0, 8.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    WgpuBuffer::write_mapped(source, 0.0, Bytes(floats.clone())).unwrap();
    WgpuBuffer::unmap(source).unwrap();
    let target =
        WgpuBuffer::create(device, json!({ "size": 16, "usage": MAP_READ | COPY_DST })).unwrap();
    let (src, dst) = (
        nsh_webgpu::api::buffer::BufferEntry::lookup(source).unwrap(),
        nsh_webgpu::api::buffer::BufferEntry::lookup(target).unwrap(),
    );
    crate::common::submit(device, |e| {
        e.copy_buffer_to_buffer(&src.buffer, 0, &dst.buffer, 0, 16)
    });
    wait(WgpuBuffer::map_async(target, MAP_READ, None, None).unwrap()).unwrap();
    assert_eq!(
        WgpuBuffer::get_mapped_range(target, None, None).unwrap(),
        floats
    );
    WgpuBuffer::unmap(target).unwrap();

    // destroy() while mapAsync waits: AbortError, then unmapped for good.
    let ticket = WgpuBuffer::map_async(target, MAP_READ, None, None).unwrap();
    WgpuBuffer::destroy(target).unwrap();
    assert_throws(wait(ticket), "AbortError");
    assert_eq!(WgpuBuffer::map_state(target).unwrap(), "unmapped");
    let (ticket, error) = validation_error(device, || {
        WgpuBuffer::map_async(target, MAP_READ, None, None).unwrap()
    });
    assert_throws(wait(ticket), "OperationError");
    assert!(error.is_some(), "mapping a destroyed buffer");
}

#[test]
fn created_mapped_buffers() {
    let device = device();
    // Buffer.spec "metadata": 1440 bytes, VERTEX, mapped at creation.
    let vertices = WgpuBuffer::create(
        device,
        json!({ "size": 1440, "usage": VERTEX, "mappedAtCreation": true, "label": "verticesBuffer" }),
    )
    .unwrap();
    assert_eq!(WgpuBuffer::map_state(vertices).unwrap(), "mapped");
    assert_eq!(
        WgpuBuffer::get_mapped_range(vertices, Some(0.0), Some(1440.0))
            .unwrap()
            .len(),
        1440
    );
    WgpuBuffer::write_mapped(vertices, 0.0, Bytes(vec![7; 1440])).unwrap();
    WgpuBuffer::unmap(vertices).unwrap();
    assert_eq!(WgpuBuffer::map_state(vertices).unwrap(), "unmapped");
    // A size that is not a multiple of 4: RangeError, nothing made.
    assert_throws(
        WgpuBuffer::create(
            device,
            json!({ "size": 6, "usage": VERTEX, "mappedAtCreation": true }),
        ),
        "RangeError",
    );
    // A bit GPUBufferUsage does not define: a validation error.
    let (_, error) = validation_error(device, || {
        WgpuBuffer::create(device, json!({ "size": 4, "usage": 0x1000 })).unwrap()
    });
    assert!(error.is_some());
    // Larger than maxBufferSize: a validation error, not a panic.
    let max = WgpuDevice::limits(device).unwrap()["maxBufferSize"]
        .as_f64()
        .unwrap() as u64;
    let (_, error) = validation_error(device, || {
        WgpuBuffer::create(device, json!({ "size": max + 4, "usage": COPY_DST })).unwrap()
    });
    assert!(error.is_some());
    // EnforceRange happens in TSX; a negative size never converts here.
    assert_throws(
        WgpuBuffer::create(device, json!({ "size": -1, "usage": COPY_DST })),
        "TypeError",
    );
}

/// Buffer.spec "writes an image into a buffer and reads it back".
#[test]
fn an_image_written_into_a_buffer_reads_back_as_the_reference() {
    let device = device();
    let mut data = vec![255u8; 256 * 256 * 4];
    // The reference's loop runs past the end; a typed array drops those
    // writes, so only the first 256 * 256 * 4 count.
    let mut i = 0usize;
    'fill: for x in 0..256 * 4 {
        for y in 0..256 * 4 {
            if i == data.len() {
                break 'fill;
            }
            data[i] = ((x * y) % 255) as u8;
            i += 1;
        }
    }
    let buffer = WgpuBuffer::create(
        device,
        json!({ "size": data.len(), "usage": MAP_READ | COPY_DST }),
    )
    .unwrap();
    WgpuQueue::write_buffer(device, buffer, 0.0, Bytes(data.clone())).unwrap();
    wait(WgpuBuffer::map_async(buffer, MAP_READ, Some(0.0), Some(data.len() as f64)).unwrap())
        .unwrap();
    let read = WgpuBuffer::get_mapped_range(buffer, Some(0.0), Some(data.len() as f64)).unwrap();
    WgpuBuffer::unmap(buffer).unwrap();
    assert_eq!(read, data);
    let image = image::RgbaImage::from_raw(256, 256, read).unwrap();
    check_image(&image, "buffer.png");
}
