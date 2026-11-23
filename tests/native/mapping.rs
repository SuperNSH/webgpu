//! Buffer mapping: copy-in/copy-out through `getMappedRange` and `unmap`.

use nsh_webgpu::api::{buffer::WgpuBuffer, command_encoder::WgpuCommandEncoder, queue::WgpuQueue};
use nsh_webgpu::bytes::Bytes;
use serde_json::json;

use crate::common::{device, wait};

/// `GPUBufferUsage`.
const MAP_READ: u32 = 0x1;
const MAP_WRITE: u32 = 0x2;
const COPY_SRC: u32 = 0x4;
const COPY_DST: u32 = 0x8;

#[test]
fn bytes_written_to_a_mapped_range_reach_the_buffer_on_unmap() {
    let device = device();
    let source = WgpuBuffer::create(
        device,
        json!({ "size": 16, "usage": COPY_SRC, "mappedAtCreation": true }),
    )
    .unwrap();
    assert_eq!(WgpuBuffer::map_state(source).unwrap(), "mapped");
    // mappedAtCreation starts zeroed.
    assert_eq!(
        WgpuBuffer::get_mapped_range(source, None, None).unwrap(),
        vec![0; 16]
    );
    WgpuBuffer::write_mapped(source, 0.0, Bytes((1..=16).collect())).unwrap();
    WgpuBuffer::unmap(source).unwrap();
    assert_eq!(WgpuBuffer::map_state(source).unwrap(), "unmapped");

    let target =
        WgpuBuffer::create(device, json!({ "size": 16, "usage": COPY_DST | MAP_READ })).unwrap();
    let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
    WgpuCommandEncoder::copy_buffer_to_buffer(encoder, source, 0.0, target, 0.0, None).unwrap();
    WgpuQueue::submit(
        device,
        vec![WgpuCommandEncoder::finish(encoder, json!(null)).unwrap()],
    )
    .unwrap();

    let ticket = WgpuBuffer::map_async(target, 1, Some(8.0), Some(8.0)).unwrap();
    assert_eq!(WgpuBuffer::map_state(target).unwrap(), "pending");
    wait(ticket).unwrap();
    assert_eq!(WgpuBuffer::map_state(target).unwrap(), "mapped");
    // Only the mapped range, and only aligned sub-ranges of it.
    assert_eq!(
        WgpuBuffer::get_mapped_range(target, Some(8.0), None).unwrap(),
        (9..=16).collect::<Vec<u8>>()
    );
    let outside = WgpuBuffer::get_mapped_range(target, Some(0.0), Some(8.0)).unwrap_err();
    assert!(outside.starts_with("OperationError:"), "{outside}");
    let misaligned = WgpuBuffer::get_mapped_range(target, Some(12.0), Some(4.0)).unwrap_err();
    assert!(misaligned.starts_with("OperationError:"), "{misaligned}");
    WgpuBuffer::unmap(target).unwrap();
    let unmapped = WgpuBuffer::get_mapped_range(target, None, None).unwrap_err();
    assert!(unmapped.starts_with("OperationError:"), "{unmapped}");
}

#[test]
fn queue_write_buffer_then_map_for_reading() {
    let device = device();
    let buffer =
        WgpuBuffer::create(device, json!({ "size": 8, "usage": COPY_DST | MAP_READ })).unwrap();
    WgpuQueue::write_buffer(device, buffer, 0.0, Bytes(vec![9, 8, 7, 6, 5, 4, 3, 2])).unwrap();
    wait(WgpuBuffer::map_async(buffer, 1, None, None).unwrap()).unwrap();
    assert_eq!(
        WgpuBuffer::get_mapped_range(buffer, None, None).unwrap(),
        vec![9, 8, 7, 6, 5, 4, 3, 2]
    );
}

#[test]
fn a_map_write_buffer_keeps_its_bytes_across_mappings() {
    let device = device();
    let buffer =
        WgpuBuffer::create(device, json!({ "size": 8, "usage": MAP_WRITE | COPY_SRC })).unwrap();
    wait(WgpuBuffer::map_async(buffer, 2, None, None).unwrap()).unwrap();
    WgpuBuffer::write_mapped(buffer, 0.0, Bytes(vec![1, 2, 3, 4, 5, 6, 7, 8])).unwrap();
    WgpuBuffer::unmap(buffer).unwrap();
    wait(WgpuBuffer::map_async(buffer, 2, None, None).unwrap()).unwrap();
    assert_eq!(
        WgpuBuffer::get_mapped_range(buffer, None, None).unwrap(),
        vec![1, 2, 3, 4, 5, 6, 7, 8]
    );
}

#[test]
fn unmap_before_the_mapping_finishes_aborts_it() {
    let device = device();
    let buffer = WgpuBuffer::create(device, json!({ "size": 8, "usage": MAP_READ })).unwrap();
    let ticket = WgpuBuffer::map_async(buffer, 1, None, None).unwrap();
    WgpuBuffer::unmap(buffer).unwrap();
    let error = wait(ticket).unwrap_err();
    assert!(error.starts_with("AbortError:"), "{error}");
    assert_eq!(WgpuBuffer::map_state(buffer).unwrap(), "unmapped");
}

#[test]
fn destroy_unmaps() {
    let device = device();
    let buffer = WgpuBuffer::create(
        device,
        json!({ "size": 8, "usage": COPY_SRC, "mappedAtCreation": true }),
    )
    .unwrap();
    WgpuBuffer::destroy(buffer).unwrap();
    assert_eq!(WgpuBuffer::map_state(buffer).unwrap(), "unmapped");
}

#[test]
fn ranges_whose_end_overflows_u64_are_rejected_not_wrapped() {
    let device = device();
    let buffer =
        WgpuBuffer::create(device, json!({ "size": 16, "usage": MAP_READ | COPY_DST })).unwrap();
    // 2^63 + 2^63 wraps to 0 in u64.
    let half = 9_223_372_036_854_775_808.0;
    let ticket = WgpuBuffer::map_async(buffer, 1, Some(half), Some(half)).unwrap();
    let error = wait(ticket).unwrap_err();
    assert!(
        error.starts_with("OperationError: mapAsync:") && error.contains("fit in 16 bytes"),
        "{error}"
    );
    let ticket = WgpuBuffer::map_async(buffer, 1, None, None).unwrap();
    wait(ticket).unwrap();
    let error = WgpuBuffer::get_mapped_range(buffer, Some(half), Some(half)).unwrap_err();
    assert!(
        error.starts_with("OperationError: getMappedRange:")
            && error.contains("outside the mapped range"),
        "{error}"
    );
}
