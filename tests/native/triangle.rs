//! The reference's Triangle (`apps/example/src/Triangle/HelloTriangle.tsx`),
//! offscreen: the same shaders, pipeline and pass, into a texture of the
//! preferred canvas format instead of the canvas, read back through a
//! buffer.

use nsh_webgpu::api::{
    buffer::WgpuBuffer, command_encoder::WgpuCommandEncoder, gpu::Wgpu, queue::WgpuQueue,
    render_pass_encoder::WgpuRenderPassEncoder, render_pipeline::WgpuRenderPipeline,
    shader_module::WgpuShaderModule, texture::WgpuTexture, texture_view::WgpuTextureView,
};
use serde_json::json;

use crate::common::{device, wait};

/// `triangle.ts` of the reference.
const TRIANGLE_VERT_WGSL: &str = "@vertex
fn main(
  @builtin(vertex_index) VertexIndex : u32
) -> @builtin(position) vec4f {
  var pos = array<vec2f, 3>(
    vec2(0.0, 0.5),
    vec2(-0.5, -0.5),
    vec2(0.5, -0.5)
  );

  return vec4f(pos[VertexIndex], 0.0, 1.0);
}";

const RED_FRAG_WGSL: &str = "@fragment
fn main() -> @location(0) vec4f {
  return vec4(1.0, 0.0, 0.0, 1.0);
}";

const SIZE: u32 = 64;
/// `GPUTextureUsage.RENDER_ATTACHMENT | COPY_SRC`.
const TEXTURE_USAGE: u32 = 0x10 | 0x01;
/// `GPUBufferUsage.COPY_DST | MAP_READ`.
const READBACK_USAGE: u32 = 0x08 | 0x01;

/// Renders the triangle into a `SIZE`² texture of `format`; its pixels,
/// 4 bytes each, rows packed.
pub(crate) fn render_triangle(format: &str) -> Vec<u8> {
    let device = device();
    let vertex = WgpuShaderModule::create(device, json!({ "code": TRIANGLE_VERT_WGSL })).unwrap();
    let fragment = WgpuShaderModule::create(device, json!({ "code": RED_FRAG_WGSL })).unwrap();
    let pipeline = WgpuRenderPipeline::create(
        device,
        json!({
            "layout": "auto",
            "vertex": { "module": vertex, "entryPoint": "main" },
            "fragment": { "module": fragment, "entryPoint": "main", "targets": [{ "format": format }] },
            "primitive": { "topology": "triangle-list" },
        }),
    )
    .unwrap();
    let texture = WgpuTexture::create(
        device,
        json!({ "size": [SIZE, SIZE], "format": format, "usage": TEXTURE_USAGE }),
    )
    .unwrap();
    let view = WgpuTextureView::create(texture, json!(null)).unwrap();
    let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
    let pass = WgpuCommandEncoder::begin_render_pass(
        encoder,
        json!({
            "colorAttachments": [{
                "view": view,
                "clearValue": [0, 0, 0, 0],
                "loadOp": "clear",
                "storeOp": "store",
            }],
        }),
    )
    .unwrap();
    WgpuRenderPassEncoder::set_pipeline(pass, pipeline).unwrap();
    WgpuRenderPassEncoder::draw(pass, 3, None, None, None).unwrap();
    WgpuRenderPassEncoder::end(pass).unwrap();
    let bytes_per_row = SIZE * 4;
    let readback = WgpuBuffer::create(
        device,
        json!({ "size": bytes_per_row * SIZE, "usage": READBACK_USAGE }),
    )
    .unwrap();
    WgpuCommandEncoder::copy_texture_to_buffer(
        encoder,
        json!({ "texture": texture }),
        json!({ "buffer": readback, "bytesPerRow": bytes_per_row }),
        json!([SIZE, SIZE]),
    )
    .unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
    let ticket = WgpuBuffer::map_async(readback, 1, None, None).unwrap();
    wait(ticket).unwrap();
    let pixels = WgpuBuffer::get_mapped_range(readback, None, None).unwrap();
    WgpuBuffer::unmap(readback).unwrap();
    for id in [
        vertex, fragment, pipeline, texture, view, encoder, pass, commands, readback,
    ] {
        Wgpu::release(id).unwrap();
    }
    pixels
}

/// The pixel at `(x, y)` as RGBA, from `bgra8unorm` or `rgba8unorm` bytes.
fn rgba(pixels: &[u8], format: &str, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * SIZE + x) * 4) as usize;
    let p = &pixels[i..i + 4];
    if format.starts_with("bgra") {
        [p[2], p[1], p[0], p[3]]
    } else {
        [p[0], p[1], p[2], p[3]]
    }
}

#[test]
fn the_reference_triangle_renders_offscreen() {
    let format = Wgpu::preferred_canvas_format();
    let pixels = render_triangle(&format);
    assert_eq!(pixels.len(), (SIZE * SIZE * 4) as usize);
    const RED: [u8; 4] = [255, 0, 0, 255];
    const CLEAR: [u8; 4] = [0, 0, 0, 0];
    // Inside: the centre, and near each corner of the triangle
    // ((0, .5), (-.5, -.5), (.5, -.5) in clip space: y grows down here).
    for (x, y) in [(32, 32), (32, 20), (20, 46), (44, 46)] {
        assert_eq!(rgba(&pixels, &format, x, y), RED, "inside at ({x}, {y})");
    }
    // Outside: the corners, beside the apex, below the base.
    for (x, y) in [
        (0, 0),
        (63, 0),
        (0, 63),
        (63, 63),
        (20, 20),
        (44, 20),
        (32, 52),
    ] {
        assert_eq!(rgba(&pixels, &format, x, y), CLEAR, "outside at ({x}, {y})");
    }
    let red = (0..SIZE * SIZE)
        .filter(|i| rgba(&pixels, &format, i % SIZE, i / SIZE) == RED)
        .count();
    // Half of the 32 × 32 bounding box of the triangle, give or take its edges.
    assert!((480..=560).contains(&red), "{red} red pixels");
    // The picture, for docs/evidence (NSH_WEBGPU_EVIDENCE=<dir>).
    if let Ok(dir) = std::env::var("NSH_WEBGPU_EVIDENCE") {
        let mut image = image::RgbaImage::new(SIZE, SIZE);
        for (x, y, p) in image.enumerate_pixels_mut() {
            *p = image::Rgba(rgba(&pixels, &format, x, y));
        }
        image
            .save(std::path::Path::new(&dir).join("triangle-rust.png"))
            .unwrap();
    }
}
