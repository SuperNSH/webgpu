//! The encoder's state machine as the spec has it: a pass locks it, a
//! call on a locked encoder invalidates it, a call on an ended one or an
//! ended pass is a validation error, `finish()` of an invalid encoder is a
//! validation error and an invalid command buffer.

use nsh_webgpu::api::{
    command_encoder::WgpuCommandEncoder as Encoder, compute_pass_encoder::WgpuComputePassEncoder,
    device::WgpuDevice, queue::WgpuQueue, render_pass_encoder::WgpuRenderPassEncoder as Pass,
};
use serde_json::json;

use crate::common::*;

fn scoped<R>(device: u32, f: impl FnOnce() -> R) -> (R, Option<String>) {
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    let r = f();
    (r, pop_error(device))
}

#[test]
fn finishing_with_a_pass_open_is_a_validation_error() {
    let device = device();
    let canvas = Canvas::sized(device, 4);
    let encoder = Encoder::create(device, json!({ "label": "e" })).unwrap();
    let _pass = Encoder::begin_render_pass(
        encoder,
        json!({ "colorAttachments": [canvas.attachment([0.0; 4])] }),
    )
    .unwrap();
    let (commands, error) = scoped(device, || Encoder::finish(encoder, json!(null)).unwrap());
    assert!(error.unwrap().contains("a pass is still open"));
    // The command buffer is invalid: submitting it is an error too.
    let (_, error) = scoped(device, || {
        WgpuQueue::submit(device, vec![commands]).unwrap()
    });
    assert!(error.unwrap().contains("invalid"));
}

#[test]
fn a_call_on_a_locked_encoder_invalidates_it() {
    let device = device();
    let canvas = Canvas::sized(device, 4);
    let buffer = buffer_with(device, usage::COPY_SRC, &[0; 16]);
    let encoder = Encoder::create(device, json!(null)).unwrap();
    let pass = Encoder::begin_render_pass(
        encoder,
        json!({ "colorAttachments": [canvas.attachment([0.0; 4])] }),
    )
    .unwrap();
    // No error yet: the encoder is invalid, not reported...
    let (_, error) = scoped(device, || {
        Encoder::clear_buffer(encoder, buffer, None, None).unwrap()
    });
    assert_eq!(error, None);
    Pass::end(pass).unwrap();
    // ...until finish().
    let (_, error) = scoped(device, || Encoder::finish(encoder, json!(null)).unwrap());
    assert!(
        error
            .unwrap()
            .contains("clearBuffer was called while a pass was open")
    );
}

#[test]
fn a_second_pass_while_one_is_open_is_invalid() {
    let device = device();
    let encoder = Encoder::create(device, json!(null)).unwrap();
    let first = Encoder::begin_compute_pass(encoder, json!(null)).unwrap();
    let second = Encoder::begin_compute_pass(encoder, json!(null)).unwrap();
    WgpuComputePassEncoder::end(second).unwrap();
    WgpuComputePassEncoder::end(first).unwrap();
    let (_, error) = scoped(device, || Encoder::finish(encoder, json!(null)).unwrap());
    assert!(error.is_some());
}

#[test]
fn calls_after_end_and_after_finish_are_validation_errors() {
    let device = device();
    let canvas = Canvas::sized(device, 4);
    let encoder = Encoder::create(device, json!(null)).unwrap();
    let pass = Encoder::begin_render_pass(
        encoder,
        json!({ "colorAttachments": [canvas.attachment([0.0; 4])] }),
    )
    .unwrap();
    Pass::end(pass).unwrap();
    let (_, error) = scoped(device, || Pass::draw(pass, 3, None, None, None).unwrap());
    assert!(error.unwrap().contains("draw: the render pass has ended"));
    let (_, error) = scoped(device, || Pass::end(pass).unwrap());
    assert!(error.unwrap().contains("already ended"));
    // The encoder itself is fine.
    let (commands, error) = scoped(device, || Encoder::finish(encoder, json!(null)).unwrap());
    assert_eq!(error, None);
    let (_, error) = scoped(device, || {
        Encoder::push_debug_group(encoder, "g".into()).unwrap()
    });
    assert!(error.unwrap().contains("finished"));
    let (_, error) = scoped(device, || Encoder::finish(encoder, json!(null)).unwrap());
    assert!(error.unwrap().contains("already finished"));
    let (_, error) = scoped(device, || {
        WgpuQueue::submit(device, vec![commands]).unwrap()
    });
    assert_eq!(error, None);
}

#[test]
fn unbalanced_debug_groups_make_finish_fail() {
    let device = device();
    let encoder = Encoder::create(device, json!(null)).unwrap();
    Encoder::push_debug_group(encoder, "open".into()).unwrap();
    Encoder::insert_debug_marker(encoder, "marker".into()).unwrap();
    let (_, error) = scoped(device, || Encoder::finish(encoder, json!(null)).unwrap());
    assert!(error.is_some(), "a debug group left open");
    let encoder = Encoder::create(device, json!(null)).unwrap();
    Encoder::push_debug_group(encoder, "open".into()).unwrap();
    Encoder::pop_debug_group(encoder).unwrap();
    let (_, error) = scoped(device, || Encoder::finish(encoder, json!(null)).unwrap());
    assert_eq!(error, None);
}

#[test]
fn an_invalid_pass_invalidates_its_encoder() {
    let device = device();
    let canvas = Canvas::sized(device, 4);
    let vertices = buffer_with(device, usage::VERTEX, &[0; 16]);
    let encoder = Encoder::create(device, json!(null)).unwrap();
    let pass = Encoder::begin_render_pass(
        encoder,
        json!({ "colorAttachments": [canvas.attachment([0.0; 4])] }),
    )
    .unwrap();
    // Past the buffer's end: wgpu would panic on the slice; it is the
    // spec's validation error at finish here.
    Pass::set_vertex_buffer(pass, 0, Some(vertices), Some(8.0), Some(16.0)).unwrap();
    Pass::end(pass).unwrap();
    let (_, error) = scoped(device, || Encoder::finish(encoder, json!(null)).unwrap());
    assert!(error.unwrap().contains("setVertexBuffer"));
}

#[test]
fn enforce_range_offsets_are_type_errors() {
    // EnforceRange.spec.ts "GPUSize64 as a direct argument".
    let device = device();
    let buffer = buffer_with(device, usage::COPY_SRC, &[0; 16]);
    let encoder = Encoder::create(device, json!(null)).unwrap();
    let e =
        Encoder::copy_buffer_to_buffer(encoder, buffer, -4.0, buffer, 0.0, Some(4.0)).unwrap_err();
    assert!(e.starts_with("TypeError:"), "{e}");
    let e = Encoder::clear_buffer(encoder, buffer, Some(f64::NAN), None).unwrap_err();
    assert!(e.starts_with("TypeError:"), "{e}");
    let e = Pass::draw_indirect(0, buffer, -1.0).unwrap_err();
    assert!(e.starts_with("TypeError:"), "{e}");
}

#[test]
fn unknown_ids_in_a_descriptor_are_type_errors() {
    let device = device();
    let encoder = Encoder::create(device, json!(null)).unwrap();
    let e = Encoder::begin_render_pass(
        encoder,
        json!({ "colorAttachments": [{ "view": 999_999, "loadOp": "clear", "storeOp": "store" }] }),
    )
    .unwrap_err();
    assert!(e.starts_with("TypeError:"), "{e}");
    let e = Encoder::begin_render_pass(encoder, json!({ "colorAttachments": [{ "view": 1 }] }))
        .unwrap_err();
    assert!(e.starts_with("TypeError:"), "missing loadOp: {e}");
}
