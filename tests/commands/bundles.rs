//! Render bundles: recorded once, executed in passes. The reference's
//! bundle demo (`demos/RenderBundles.spec.ts`, the asteroid field) needs
//! samplers and bind groups (area (a)); these draw its other scenes
//! through bundles and compare with the same snapshots.

use nsh_webgpu::api::{
    command_encoder::WgpuCommandEncoder, device::WgpuDevice, queue::WgpuQueue,
    render_bundle::WgpuRenderBundle, render_bundle_encoder::WgpuRenderBundleEncoder as Bundle,
    render_pass_encoder::WgpuRenderPassEncoder as Pass,
};
use serde_json::json;

use crate::common::*;

fn run_bundles(canvas: &Canvas, bundles: Vec<u32>) -> Option<String> {
    run_bundles_on(canvas, bundles, [0.0, 0.0, 0.0, 1.0])
}

fn run_bundles_on(canvas: &Canvas, bundles: Vec<u32>, clear: [f64; 4]) -> Option<String> {
    let device = canvas.device;
    let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
    let pass = WgpuCommandEncoder::begin_render_pass(
        encoder,
        json!({ "colorAttachments": [canvas.attachment(clear)] }),
    )
    .unwrap();
    Pass::execute_bundles(pass, bundles).unwrap();
    Pass::end(pass).unwrap();
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
    pop_error(device)
}

#[test]
fn a_bundle_draws_the_triangle() {
    let device = device();
    let canvas = Canvas::new(device);
    let p = pipeline(
        device,
        TRIANGLE_VERT_WGSL,
        RED_FRAG_WGSL,
        &canvas.format,
        json!({}),
    );
    let encoder = Bundle::create(device, json!({ "colorFormats": [canvas.format] })).unwrap();
    Bundle::set_pipeline(encoder, p).unwrap();
    Bundle::draw(encoder, 3, None, None, None).unwrap();
    let bundle = Bundle::finish(encoder, json!({ "label": "triangle" })).unwrap();
    assert!(WgpuRenderBundle::is_valid(bundle).unwrap());
    assert_eq!(
        run_bundles_on(&canvas, vec![bundle], [0.3, 0.6, 1.0, 1.0]),
        None
    );
    check_image(&canvas.rgba(), 1024, 1024, "triangle.png");
}

#[test]
fn bundles_replay_indexed_and_vertex_buffer_draws() {
    // demos/DrawInstanced.spec.ts "drawIndexed with an instance count",
    // through a bundle executed twice in one pass.
    const QUAD: &str = "struct O { @builtin(position) p: vec4f, @location(0) @interpolate(flat) i: u32 };
    @vertex fn vs(@location(0) pos: vec2f, @builtin(instance_index) i: u32) -> O {
      let center = vec2f(-2.0 / 3.0 + f32(i % 3u) * (2.0 / 3.0), 2.0 / 3.0 - f32(i / 3u) * (2.0 / 3.0));
      return O(vec4f(pos + center, 0.0, 1.0), i);
    }
    @fragment fn fs(@location(0) @interpolate(flat) i: u32) -> @location(0) vec4f {
      let c = array<vec3f, 9>(vec3f(1, 0, 0), vec3f(0, 1, 0), vec3f(0, 0, 1), vec3f(1, 1, 0), vec3f(1, 0, 1),
        vec3f(0, 1, 1), vec3f(1, 0.5, 0), vec3f(0.5, 0, 1), vec3f(1, 1, 1));
      return vec4f(c[i % 9u], 1.0);
    }";
    let device = device();
    let canvas = Canvas::new(device);
    let p = pipeline(
        device,
        QUAD,
        QUAD,
        &canvas.format,
        json!({ "vertex": { "buffers": [{ "arrayStride": 8, "attributes": [
            { "shaderLocation": 0, "offset": 0, "format": "float32x2" },
        ] }] } }),
    );
    let vertices = buffer_with(
        device,
        usage::VERTEX,
        &f32s(&[-0.2, 0.2, -0.2, -0.2, 0.2, -0.2, 0.2, 0.2]),
    );
    let indices: Vec<u8> = [0u16, 1, 2, 0, 2, 3]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let index = buffer_with(device, usage::INDEX, &indices);
    let encoder = Bundle::create(device, json!({ "colorFormats": [canvas.format] })).unwrap();
    Bundle::set_pipeline(encoder, p).unwrap();
    Bundle::set_vertex_buffer(encoder, 0, Some(vertices), None, None).unwrap();
    Bundle::set_index_buffer(encoder, index, "uint16".into(), None, None).unwrap();
    Bundle::draw_indexed(encoder, 6, Some(9), None, None, None).unwrap();
    let bundle = Bundle::finish(encoder, json!(null)).unwrap();
    assert_eq!(run_bundles(&canvas, vec![bundle, bundle]), None);
    check_image(&canvas.rgba(), 1024, 1024, "draw-indexed-instanced.png");
}

#[test]
fn an_invalid_descriptor_is_a_validation_error_and_an_invalid_bundle() {
    let device = device();
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    // No attachment at all.
    let encoder = Bundle::create(device, json!({ "colorFormats": [] })).unwrap();
    assert!(pop_error(device).is_some());
    let bundle = Bundle::finish(encoder, json!(null)).unwrap();
    assert!(!WgpuRenderBundle::is_valid(bundle).unwrap());
    // Executing it makes the pass, and so the encoder, invalid.
    let canvas = Canvas::sized(device, 4);
    assert!(
        run_bundles(&canvas, vec![bundle])
            .unwrap()
            .contains("invalid")
    );
}

#[test]
fn a_bundle_of_the_wrong_format_fails_the_pass_and_finishing_twice_fails() {
    let device = device();
    let canvas = Canvas::sized(device, 4);
    let encoder = Bundle::create(device, json!({ "colorFormats": ["rgba32float"] })).unwrap();
    let bundle = Bundle::finish(encoder, json!(null)).unwrap();
    assert!(WgpuRenderBundle::is_valid(bundle).unwrap());
    assert!(
        run_bundles(&canvas, vec![bundle]).is_some(),
        "incompatible with the pass"
    );
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    Bundle::finish(encoder, json!(null)).unwrap();
    assert!(pop_error(device).unwrap().contains("already finished"));
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    Bundle::draw(encoder, 3, None, None, None).unwrap();
    assert!(pop_error(device).unwrap().contains("has ended"));
}
