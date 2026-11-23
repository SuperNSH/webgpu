//! GPU errors reach error scopes and `uncapturederror` as the spec says,
//! and exceptions cross with their class.

use nsh_webgpu::api::{
    buffer::WgpuBuffer, command_encoder::WgpuCommandEncoder, device::WgpuDevice, queue::WgpuQueue,
    shader_module::WgpuShaderModule,
};
use serde_json::json;

use crate::common::device;

#[test]
fn a_validation_error_goes_to_the_innermost_matching_scope() {
    let device = device();
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    WgpuDevice::push_error_scope(device, "out-of-memory".into()).unwrap();
    // MAP_READ with MAP_WRITE is invalid.
    WgpuBuffer::create(device, json!({ "size": 4, "usage": 3 })).unwrap();
    // The out-of-memory scope does not catch it...
    assert_eq!(WgpuDevice::pop_error_scope(device).unwrap(), json!(null));
    // ...the validation one does, once (the first error only).
    WgpuBuffer::create(device, json!({ "size": 4, "usage": 0 })).unwrap();
    let error = WgpuDevice::pop_error_scope(device).unwrap();
    assert_eq!(error["kind"], "validation");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("usage"),
        "{error}"
    );
    // No scope left: popping is an OperationError.
    let empty = WgpuDevice::pop_error_scope(device).unwrap_err();
    assert!(empty.starts_with("OperationError:"), "{empty}");
    let filter = WgpuDevice::push_error_scope(device, "oops".into()).unwrap_err();
    assert!(filter.starts_with("TypeError:"), "{filter}");
}

#[test]
fn invalid_wgsl_is_a_validation_error_with_compilation_messages() {
    let device = device();
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    let module = WgpuShaderModule::create(device, json!({ "code": "fn main( {" })).unwrap();
    let error = WgpuDevice::pop_error_scope(device).unwrap();
    assert_eq!(error["kind"], "validation", "{error}");
    let messages = WgpuShaderModule::compilation_info(module).unwrap();
    assert!(
        messages.iter().any(|m| m["type"] == "error"),
        "{messages:?}"
    );
}

#[test]
fn an_error_no_scope_catches_is_uncaptured_not_a_panic() {
    let device = device();
    // No listener on this thread: it waits for TSX to take it.
    WgpuBuffer::create(device, json!({ "size": 4, "usage": 3 })).unwrap();
    let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
    // Submitting it again is invalid.
    WgpuQueue::submit(device, vec![commands]).unwrap();
    let errors = WgpuDevice::take_uncaptured_errors(device).unwrap();
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(errors.iter().all(|e| e["kind"] == "validation"));
    assert!(
        errors[1]["message"]
            .as_str()
            .unwrap()
            .contains("submitted before")
    );
}

#[test]
fn a_descriptor_that_does_not_convert_is_a_type_error() {
    let device = device();
    let error = WgpuBuffer::create(device, json!({ "usage": 4 })).unwrap_err();
    assert!(error.starts_with("TypeError:"), "{error}");
    let error = WgpuBuffer::create(
        device,
        json!({ "size": 6, "usage": 4, "mappedAtCreation": true }),
    )
    .unwrap_err();
    assert!(error.starts_with("RangeError:"), "{error}");
}

#[test]
fn destroy_loses_the_device() {
    let device = device();
    assert_eq!(WgpuDevice::lost_info(device).unwrap(), json!(null));
    WgpuDevice::destroy(device).unwrap();
    assert_eq!(
        WgpuDevice::lost_info(device).unwrap()["reason"],
        "destroyed"
    );
}

#[test]
fn destroy_releases_the_device_and_release_drops_it() {
    use nsh_webgpu::api::gpu::Wgpu;
    let device = device();
    let buffer = WgpuBuffer::create(device, json!({ "size": 16, "usage": 8 })).unwrap();
    assert!(Wgpu::alive(device));
    WgpuDevice::destroy(device).unwrap();
    // Out of the registry: its id is released...
    assert!(!Wgpu::alive(device));
    // ...but its objects still find it, as a lost device's objects do.
    assert_eq!(
        WgpuDevice::lost_info(device).unwrap()["reason"],
        "destroyed"
    );
    WgpuDevice::destroy(device).unwrap();
    WgpuBuffer::destroy(buffer).unwrap();
    // Its errors go nowhere once it is lost.
    WgpuBuffer::create(device, json!({ "size": 4, "usage": 3 })).unwrap();
    assert_eq!(
        WgpuDevice::take_uncaptured_errors(device).unwrap(),
        Vec::<serde_json::Value>::new()
    );
    // TSX collected the GPUDevice: nothing of it is left.
    WgpuDevice::release(device).unwrap();
    assert!(WgpuDevice::lost_info(device).is_err());
}
