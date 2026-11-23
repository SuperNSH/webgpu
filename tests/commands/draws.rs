//! Render passes against the reference's snapshots: `Alpha.spec.ts`,
//! `demos/Triangle.spec.ts` (simple, MSAA), `Texture.spec.ts` ("Create
//! texture and reads it"), `demos/DrawInstanced.spec.ts` (all five), and
//! the pass state the spec has beyond them (viewport, scissor, blend
//! constant, stencil reference, debug groups, null vertex buffers).

use nsh_webgpu::api::{
    command_encoder::WgpuCommandEncoder, device::WgpuDevice, queue::WgpuQueue,
    render_pass_encoder::WgpuRenderPassEncoder as Pass, texture::WgpuTexture,
    texture_view::WgpuTextureView,
};
use serde_json::json;

use crate::common::*;

/// Begins a pass clearing `canvas` to `clear`; `(encoder, pass)`.
fn begin(canvas: &Canvas, clear: [f64; 4]) -> (u32, u32) {
    let encoder = WgpuCommandEncoder::create(canvas.device, json!(null)).unwrap();
    let pass = WgpuCommandEncoder::begin_render_pass(
        encoder,
        json!({ "colorAttachments": [canvas.attachment(clear)] }),
    )
    .unwrap();
    (encoder, pass)
}

/// Ends `pass`, finishes and submits `encoder`.
fn submit(device: u32, encoder: u32, pass: u32) {
    Pass::end(pass).unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
}

#[test]
fn alpha_premultiplied_color() {
    // Alpha.spec.ts: a pass that only clears to premultiplied cyan.
    let device = device();
    let canvas = Canvas::new(device);
    let alpha = 0.5;
    let (encoder, pass) = begin(&canvas, [0.3 * alpha, 0.6 * alpha, alpha, alpha]);
    submit(device, encoder, pass);
    check_image(&canvas.rgba(), 1024, 1024, "semi-opaque-cyan.png");
}

#[test]
fn simple_triangle() {
    // demos/Triangle.spec.ts "Simple Triangle".
    let device = device();
    let canvas = Canvas::new(device);
    let p = pipeline(
        device,
        TRIANGLE_VERT_WGSL,
        RED_FRAG_WGSL,
        &canvas.format,
        json!({}),
    );
    let (encoder, pass) = begin(&canvas, [0.3, 0.6, 1.0, 1.0]);
    Pass::set_pipeline(pass, p).unwrap();
    Pass::draw(pass, 3, None, None, None).unwrap();
    submit(device, encoder, pass);
    check_image(&canvas.rgba(), 1024, 1024, "triangle.png");
}

#[test]
fn create_texture_and_read_it() {
    // Texture.spec.ts "Create texture and reads it": the triangle on a
    // transparent clear.
    let device = device();
    let canvas = Canvas::new(device);
    let p = pipeline(
        device,
        TRIANGLE_VERT_WGSL,
        RED_FRAG_WGSL,
        &canvas.format,
        json!({}),
    );
    let (encoder, pass) = begin(&canvas, [0.0, 0.0, 0.0, 0.0]);
    Pass::set_pipeline(pass, p).unwrap();
    Pass::draw(pass, 3, None, None, None).unwrap();
    submit(device, encoder, pass);
    check_image(&canvas.rgba(), 1024, 1024, "texture.png");
}

#[test]
fn triangle_msaa_resolves_into_the_canvas() {
    // demos/Triangle.spec.ts "Triangle MSAA": 4 samples, resolveTarget,
    // storeOp "discard".
    let device = device();
    let canvas = Canvas::new(device);
    let p = pipeline(
        device,
        TRIANGLE_VERT_WGSL,
        RED_FRAG_WGSL,
        &canvas.format,
        json!({ "multisample": { "count": 4 } }),
    );
    let msaa = WgpuTexture::create(
        device,
        json!({ "size": [1024, 1024], "sampleCount": 4, "format": canvas.format, "usage": usage::T_RENDER_ATTACHMENT }),
    )
    .unwrap();
    let view = WgpuTextureView::create(msaa, json!(null)).unwrap();
    let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
    let pass = WgpuCommandEncoder::begin_render_pass(
        encoder,
        json!({ "colorAttachments": [{
            "view": view,
            "resolveTarget": canvas.view(),
            "clearValue": [0, 0, 0, 1],
            "loadOp": "clear",
            "storeOp": "discard",
        }] }),
    )
    .unwrap();
    Pass::set_pipeline(pass, p).unwrap();
    Pass::draw(pass, 3, None, None, None).unwrap();
    submit(device, encoder, pass);
    check_image(&canvas.rgba(), 1024, 1024, "triangle-msaa.png");
}

/// demos/DrawInstanced.spec.ts: one triangle per instance on a 3x3 grid.
const INSTANCED_TRIANGLE: &str = "struct VertexOutput {
  @builtin(position) position: vec4f,
  @location(0) @interpolate(flat) instance: u32,
};

@vertex
fn vs(
  @builtin(vertex_index) vertexIndex: u32,
  @builtin(instance_index) instanceIndex: u32
) -> VertexOutput {
  let pos = array(
    vec2f( 0.0,   0.25),
    vec2f(-0.25, -0.25),
    vec2f( 0.25, -0.25)
  );
  let col = f32(instanceIndex % 3u);
  let row = f32(instanceIndex / 3u);
  let center = vec2f(
    -2.0 / 3.0 + col * (2.0 / 3.0),
     2.0 / 3.0 - row * (2.0 / 3.0)
  );
  var output: VertexOutput;
  output.position = vec4f(pos[vertexIndex] * 0.9 + center, 0.0, 1.0);
  output.instance = instanceIndex;
  return output;
}

@fragment
fn fs(@location(0) @interpolate(flat) instance: u32) -> @location(0) vec4f {
  let colors = array<vec3f, 9>(
    vec3f(1.0, 0.0, 0.0),
    vec3f(0.0, 1.0, 0.0),
    vec3f(0.0, 0.0, 1.0),
    vec3f(1.0, 1.0, 0.0),
    vec3f(1.0, 0.0, 1.0),
    vec3f(0.0, 1.0, 1.0),
    vec3f(1.0, 0.5, 0.0),
    vec3f(0.5, 0.0, 1.0),
    vec3f(1.0, 1.0, 1.0)
  );
  return vec4f(colors[instance % 9u], 1.0);
}
";

/// The same grid of quads, for `drawIndexed`.
const INSTANCED_QUAD: &str = "struct VertexOutput {
  @builtin(position) position: vec4f,
  @location(0) @interpolate(flat) instance: u32,
};

@vertex
fn vs(
  @builtin(vertex_index) vertexIndex: u32,
  @builtin(instance_index) instanceIndex: u32
) -> VertexOutput {
  let pos = array(
    vec2f(-0.2,  0.2),
    vec2f(-0.2, -0.2),
    vec2f( 0.2, -0.2),
    vec2f( 0.2,  0.2)
  );
  let col = f32(instanceIndex % 3u);
  let row = f32(instanceIndex / 3u);
  let center = vec2f(
    -2.0 / 3.0 + col * (2.0 / 3.0),
     2.0 / 3.0 - row * (2.0 / 3.0)
  );
  var output: VertexOutput;
  output.position = vec4f(pos[vertexIndex] + center, 0.0, 1.0);
  output.instance = instanceIndex;
  return output;
}

@fragment
fn fs(@location(0) @interpolate(flat) instance: u32) -> @location(0) vec4f {
  let colors = array<vec3f, 9>(
    vec3f(1.0, 0.0, 0.0),
    vec3f(0.0, 1.0, 0.0),
    vec3f(0.0, 0.0, 1.0),
    vec3f(1.0, 1.0, 0.0),
    vec3f(1.0, 0.0, 1.0),
    vec3f(0.0, 1.0, 1.0),
    vec3f(1.0, 0.5, 0.0),
    vec3f(0.5, 0.0, 1.0),
    vec3f(1.0, 1.0, 1.0)
  );
  return vec4f(colors[instance % 9u], 1.0);
}
";

/// Per-instance data from a `stepMode: "instance"` vertex buffer.
const INSTANCE_STEP_MODE: &str = "struct VertexOutput {
  @builtin(position) position: vec4f,
  @location(0) color: vec4f,
};

@vertex
fn vs(
  @location(0) position: vec2f,
  @location(1) offset: vec2f,
  @location(2) color: vec4f
) -> VertexOutput {
  var output: VertexOutput;
  output.position = vec4f(position + offset, 0.0, 1.0);
  output.color = color;
  return output;
}

@fragment
fn fs(@location(0) color: vec4f) -> @location(0) vec4f {
  return color;
}
";

#[test]
fn draw_with_an_instance_count() {
    let device = device();
    let canvas = Canvas::new(device);
    let p = pipeline(
        device,
        INSTANCED_TRIANGLE,
        INSTANCED_TRIANGLE,
        &canvas.format,
        json!({}),
    );
    let (encoder, pass) = begin(&canvas, [0.0, 0.0, 0.0, 1.0]);
    Pass::set_pipeline(pass, p).unwrap();
    Pass::draw(pass, 3, Some(9), None, None).unwrap();
    submit(device, encoder, pass);
    check_image(&canvas.rgba(), 1024, 1024, "draw-instanced.png");
}

#[test]
fn draw_with_a_non_zero_first_instance() {
    let device = device();
    let canvas = Canvas::new(device);
    let p = pipeline(
        device,
        INSTANCED_TRIANGLE,
        INSTANCED_TRIANGLE,
        &canvas.format,
        json!({}),
    );
    let (encoder, pass) = begin(&canvas, [0.0, 0.0, 0.0, 1.0]);
    Pass::set_pipeline(pass, p).unwrap();
    Pass::draw(pass, 3, Some(6), Some(0), Some(3)).unwrap();
    Pass::end(pass).unwrap();
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
    assert_eq!(pop_error(device), None);
    let rgba = canvas.rgba();
    assert!(rgba.iter().any(|&v| v != 0));
    check_image(&rgba, 1024, 1024, "draw-instanced-first-instance.png");
}

#[test]
fn draw_indexed_with_an_instance_count() {
    let device = device();
    let canvas = Canvas::new(device);
    let p = pipeline(
        device,
        INSTANCED_QUAD,
        INSTANCED_QUAD,
        &canvas.format,
        json!({}),
    );
    let indices: Vec<u8> = [0u16, 1, 2, 0, 2, 3]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let index = buffer_with(device, usage::INDEX, &indices);
    let (encoder, pass) = begin(&canvas, [0.0, 0.0, 0.0, 1.0]);
    Pass::set_pipeline(pass, p).unwrap();
    Pass::set_index_buffer(pass, index, "uint16".into(), None, None).unwrap();
    Pass::draw_indexed(pass, 6, Some(9), None, None, None).unwrap();
    submit(device, encoder, pass);
    check_image(&canvas.rgba(), 1024, 1024, "draw-indexed-instanced.png");
}

#[test]
fn draw_with_a_step_mode_instance_vertex_buffer() {
    let device = device();
    let canvas = Canvas::new(device);
    let p = pipeline(
        device,
        INSTANCE_STEP_MODE,
        INSTANCE_STEP_MODE,
        &canvas.format,
        json!({ "vertex": { "buffers": [
            { "arrayStride": 8, "stepMode": "vertex", "attributes": [
                { "shaderLocation": 0, "offset": 0, "format": "float32x2" },
            ] },
            { "arrayStride": 24, "stepMode": "instance", "attributes": [
                { "shaderLocation": 1, "offset": 0, "format": "float32x2" },
                { "shaderLocation": 2, "offset": 8, "format": "float32x4" },
            ] },
        ] } }),
    );
    let vertices = buffer_with(
        device,
        usage::VERTEX,
        &f32s(&[0.0, 0.3, -0.3, -0.3, 0.3, -0.3]),
    );
    #[rustfmt::skip]
    let instances = buffer_with(device, usage::VERTEX, &f32s(&[
        -0.5,  0.5, 1.0, 0.0, 0.0, 1.0,
         0.5,  0.5, 0.0, 1.0, 0.0, 1.0,
        -0.5, -0.5, 0.0, 0.0, 1.0, 1.0,
         0.5, -0.5, 1.0, 1.0, 0.0, 1.0,
    ]));
    let (encoder, pass) = begin(&canvas, [0.0, 0.0, 0.0, 1.0]);
    Pass::set_pipeline(pass, p).unwrap();
    Pass::set_vertex_buffer(pass, 0, Some(vertices), None, None).unwrap();
    Pass::set_vertex_buffer(pass, 1, Some(instances), None, None).unwrap();
    Pass::draw(pass, 3, Some(4), None, None).unwrap();
    submit(device, encoder, pass);
    check_image(&canvas.rgba(), 1024, 1024, "draw-instanced-step-mode.png");
}

#[test]
fn draw_indirect_with_an_instance_count() {
    let device = device();
    let canvas = Canvas::new(device);
    let p = pipeline(
        device,
        INSTANCED_TRIANGLE,
        INSTANCED_TRIANGLE,
        &canvas.format,
        json!({}),
    );
    let indirect = buffer_with(device, usage::INDIRECT, &u32s(&[3, 9, 0, 0]));
    let (encoder, pass) = begin(&canvas, [0.0, 0.0, 0.0, 1.0]);
    Pass::set_pipeline(pass, p).unwrap();
    Pass::draw_indirect(pass, indirect, 0.0).unwrap();
    submit(device, encoder, pass);
    check_image(&canvas.rgba(), 1024, 1024, "draw-instanced.png");
}

#[test]
fn draw_indexed_indirect_matches_the_direct_draw() {
    let device = device();
    let canvas = Canvas::new(device);
    let p = pipeline(
        device,
        INSTANCED_QUAD,
        INSTANCED_QUAD,
        &canvas.format,
        json!({}),
    );
    let indices: Vec<u8> = [0u16, 1, 2, 0, 2, 3]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let index = buffer_with(device, usage::INDEX, &indices);
    // indexCount, instanceCount, firstIndex, baseVertex, firstInstance.
    let indirect = buffer_with(device, usage::INDIRECT, &u32s(&[6, 9, 0, 0, 0]));
    let (encoder, pass) = begin(&canvas, [0.0, 0.0, 0.0, 1.0]);
    Pass::set_pipeline(pass, p).unwrap();
    Pass::set_index_buffer(pass, index, "uint16".into(), Some(0.0), Some(12.0)).unwrap();
    Pass::draw_indexed_indirect(pass, indirect, 0.0).unwrap();
    submit(device, encoder, pass);
    check_image(&canvas.rgba(), 1024, 1024, "draw-indexed-instanced.png");
}

/// A 64 x 64 canvas; the RGBA pixel at (x, y).
fn at(rgba: &[u8], x: usize, y: usize) -> [u8; 4] {
    let i = (y * 64 + x) * 4;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

#[test]
fn viewport_and_scissor_limit_the_draw() {
    let device = device();
    let canvas = Canvas::sized(device, 64);
    // A full-screen triangle pair: two triangles covering clip space.
    const FULL: &str = "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
        var p = array(vec2f(-1, -1), vec2f(3, -1), vec2f(-1, 3));
        return vec4f(p[i], 0, 1);
    }
    @fragment fn fs() -> @location(0) vec4f { return vec4f(0, 1, 0, 1); }";
    let p = pipeline(device, FULL, FULL, &canvas.format, json!({}));
    let (encoder, pass) = begin(&canvas, [0.0, 0.0, 0.0, 1.0]);
    Pass::set_pipeline(pass, p).unwrap();
    // The left half by viewport, then its top half by scissor.
    Pass::set_viewport(pass, 0.0, 0.0, 32.0, 64.0, 0.0, 1.0).unwrap();
    Pass::set_scissor_rect(pass, 0, 0, 64, 32).unwrap();
    Pass::push_debug_group(pass, "draw".into()).unwrap();
    Pass::insert_debug_marker(pass, "here".into()).unwrap();
    Pass::draw(pass, 3, None, None, None).unwrap();
    Pass::pop_debug_group(pass).unwrap();
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    submit(device, encoder, pass);
    assert_eq!(pop_error(device), None);
    let rgba = canvas.rgba();
    assert_eq!(at(&rgba, 10, 10), [0, 255, 0, 255], "inside both");
    assert_eq!(at(&rgba, 40, 10), [0, 0, 0, 255], "outside the viewport");
    assert_eq!(at(&rgba, 10, 40), [0, 0, 0, 255], "outside the scissor");
}

#[test]
fn blend_constant_and_stencil_reference_reach_the_pipeline() {
    let device = device();
    let canvas = Canvas::sized(device, 64);
    const FULL: &str = "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
        var p = array(vec2f(-1, -1), vec2f(3, -1), vec2f(-1, 3));
        return vec4f(p[i], 0, 1);
    }
    @fragment fn fs() -> @location(0) vec4f { return vec4f(1, 1, 1, 1); }";
    // color = src * constant: the blend constant is what lands.
    let p = pipeline(
        device,
        FULL,
        FULL,
        &canvas.format,
        json!({
            "fragment": { "targets": [{ "format": canvas.format, "blend": {
                "color": { "srcFactor": "constant", "dstFactor": "zero", "operation": "add" },
                "alpha": { "srcFactor": "one", "dstFactor": "zero", "operation": "add" },
            } }] },
            "depthStencil": {
                "format": "stencil8",
                "depthWriteEnabled": false,
                "depthCompare": "always",
                "stencilFront": { "compare": "equal" },
                "stencilBack": { "compare": "equal" },
            },
        }),
    );
    let stencil = WgpuTexture::create(
        device,
        json!({ "size": [64, 64], "format": "stencil8", "usage": usage::T_RENDER_ATTACHMENT }),
    )
    .unwrap();
    let stencil_view = WgpuTextureView::create(stencil, json!(null)).unwrap();
    let draw_with = |reference: u32| {
        let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
        let pass = WgpuCommandEncoder::begin_render_pass(
            encoder,
            json!({
                "colorAttachments": [canvas.attachment([0.0, 0.0, 0.0, 1.0])],
                "depthStencilAttachment": {
                    "view": stencil_view,
                    "stencilClearValue": 7,
                    "stencilLoadOp": "clear",
                    "stencilStoreOp": "store",
                },
            }),
        )
        .unwrap();
        Pass::set_pipeline(pass, p).unwrap();
        Pass::set_blend_constant(pass, json!({ "r": 0.0, "g": 0.0, "b": 1.0, "a": 1.0 })).unwrap();
        Pass::set_stencil_reference(pass, reference).unwrap();
        Pass::draw(pass, 3, None, None, None).unwrap();
        WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
        submit(device, encoder, pass);
        assert_eq!(pop_error(device), None);
        at(&canvas.rgba(), 5, 5)
    };
    assert_eq!(
        draw_with(7),
        [0, 0, 255, 255],
        "stencil passes: the blend constant"
    );
    assert_eq!(draw_with(3), [0, 0, 0, 255], "stencil fails: the clear");
}

#[test]
fn a_depth_attachment_without_its_ops_makes_the_encoder_invalid() {
    let device = device();
    let canvas = Canvas::sized(device, 64);
    let depth = WgpuTexture::create(
        device,
        json!({ "size": [64, 64], "format": "depth24plus", "usage": usage::T_RENDER_ATTACHMENT }),
    )
    .unwrap();
    let view = WgpuTextureView::create(depth, json!(null)).unwrap();
    let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
    let pass = WgpuCommandEncoder::begin_render_pass(
        encoder,
        json!({
            "colorAttachments": [canvas.attachment([0.0, 0.0, 0.0, 1.0])],
            // depthLoadOp "clear" with no depthClearValue.
            "depthStencilAttachment": { "view": view, "depthLoadOp": "clear", "depthStoreOp": "store" },
        }),
    )
    .unwrap();
    Pass::end(pass).unwrap();
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    let error = pop_error(device).expect("a validation error at finish");
    assert!(error.contains("depthClearValue"), "{error}");
}

#[test]
fn a_null_vertex_buffer_unbinds_a_slot_the_pipeline_does_not_read() {
    // NullSlots.spec.ts's vertex-buffer cases need a hole in the pipeline's
    // `buffers`, which wgpu-core 29 drops from the HAL pipeline without
    // renumbering the slots after it (device/resource.rs: `if
    // vb_state.attributes.is_empty() { continue; }`): the draw then reads
    // slot 0's buffer for slot 1's attributes. Here the null binding itself:
    // `setVertexBuffer(1, null)` on a pipeline that reads slot 0 only.
    let device = device();
    let canvas = Canvas::sized(device, 64);
    const SHADER: &str = "@vertex fn vs(@location(0) p: vec2f) -> @builtin(position) vec4f {
        return vec4f(p, 0, 1);
    }
    @fragment fn fs() -> @location(0) vec4f { return vec4f(1, 0, 0, 1); }";
    let p = pipeline(
        device,
        SHADER,
        SHADER,
        &canvas.format,
        json!({ "vertex": { "buffers": [{ "arrayStride": 8, "attributes": [
            { "shaderLocation": 0, "offset": 0, "format": "float32x2" },
        ] }] } }),
    );
    let vertices = buffer_with(
        device,
        usage::VERTEX,
        &f32s(&[-1.0, -1.0, 3.0, -1.0, -1.0, 3.0]),
    );
    let (encoder, pass) = begin(&canvas, [0.0, 0.0, 0.0, 1.0]);
    Pass::set_pipeline(pass, p).unwrap();
    Pass::set_vertex_buffer(pass, 0, Some(vertices), None, None).unwrap();
    Pass::set_vertex_buffer(pass, 1, None, None, None).unwrap();
    Pass::draw(pass, 3, None, None, None).unwrap();
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    submit(device, encoder, pass);
    assert_eq!(pop_error(device), None);
    assert_eq!(at(&canvas.rgba(), 32, 32), [255, 0, 0, 255]);
    // Unbinding a slot the pipeline reads fails the draw, as in the spec.
    let (encoder, pass) = begin(&canvas, [0.0, 0.0, 0.0, 1.0]);
    Pass::set_pipeline(pass, p).unwrap();
    Pass::set_vertex_buffer(pass, 0, Some(vertices), None, None).unwrap();
    Pass::set_vertex_buffer(pass, 0, None, None, None).unwrap();
    Pass::draw(pass, 3, None, None, None).unwrap();
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    submit(device, encoder, pass);
    assert!(pop_error(device).is_some());
}
