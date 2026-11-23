//! `GPUTexture`, `GPUTextureView`: every format, dimension and view, the
//! reference's Texture.spec and Formats.spec.

use nsh_webgpu::api::{
    adapter::WgpuAdapter,
    buffer::{BufferEntry, WgpuBuffer},
    command_encoder::WgpuCommandEncoder,
    gpu::Wgpu,
    queue::WgpuQueue,
    render_pass_encoder::WgpuRenderPassEncoder,
    render_pipeline::WgpuRenderPipeline,
    shader_module::WgpuShaderModule,
    texture::{TextureEntry, WgpuTexture},
    texture_view::WgpuTextureView,
};
use serde_json::{Value, json};

use crate::common::{adapter, assert_throws, block, check_image, device, validation_error, wait};

const COPY_SRC: u32 = 0x01;
const COPY_DST: u32 = 0x02;
const TEXTURE_BINDING: u32 = 0x04;
const RENDER_ATTACHMENT: u32 = 0x10;

/// Every `GPUTextureFormat` of the spec (@webgpu/types 0.1.74).
pub(crate) const SPEC_FORMATS: &[&str] = &[
    "r8unorm",
    "r8snorm",
    "r8uint",
    "r8sint",
    "r16unorm",
    "r16snorm",
    "r16uint",
    "r16sint",
    "r16float",
    "rg8unorm",
    "rg8snorm",
    "rg8uint",
    "rg8sint",
    "r32uint",
    "r32sint",
    "r32float",
    "rg16unorm",
    "rg16snorm",
    "rg16uint",
    "rg16sint",
    "rg16float",
    "rgba8unorm",
    "rgba8unorm-srgb",
    "rgba8snorm",
    "rgba8uint",
    "rgba8sint",
    "bgra8unorm",
    "bgra8unorm-srgb",
    "rgb9e5ufloat",
    "rgb10a2uint",
    "rgb10a2unorm",
    "rg11b10ufloat",
    "rg32uint",
    "rg32sint",
    "rg32float",
    "rgba16unorm",
    "rgba16snorm",
    "rgba16uint",
    "rgba16sint",
    "rgba16float",
    "rgba32uint",
    "rgba32sint",
    "rgba32float",
    "stencil8",
    "depth16unorm",
    "depth24plus",
    "depth24plus-stencil8",
    "depth32float",
    "depth32float-stencil8",
    "bc1-rgba-unorm",
    "bc1-rgba-unorm-srgb",
    "bc2-rgba-unorm",
    "bc2-rgba-unorm-srgb",
    "bc3-rgba-unorm",
    "bc3-rgba-unorm-srgb",
    "bc4-r-unorm",
    "bc4-r-snorm",
    "bc5-rg-unorm",
    "bc5-rg-snorm",
    "bc6h-rgb-ufloat",
    "bc6h-rgb-float",
    "bc7-rgba-unorm",
    "bc7-rgba-unorm-srgb",
    "etc2-rgb8unorm",
    "etc2-rgb8unorm-srgb",
    "etc2-rgb8a1unorm",
    "etc2-rgb8a1unorm-srgb",
    "etc2-rgba8unorm",
    "etc2-rgba8unorm-srgb",
    "eac-r11unorm",
    "eac-r11snorm",
    "eac-rg11unorm",
    "eac-rg11snorm",
    "astc-4x4-unorm",
    "astc-4x4-unorm-srgb",
    "astc-5x4-unorm",
    "astc-5x4-unorm-srgb",
    "astc-5x5-unorm",
    "astc-5x5-unorm-srgb",
    "astc-6x5-unorm",
    "astc-6x5-unorm-srgb",
    "astc-6x6-unorm",
    "astc-6x6-unorm-srgb",
    "astc-8x5-unorm",
    "astc-8x5-unorm-srgb",
    "astc-8x6-unorm",
    "astc-8x6-unorm-srgb",
    "astc-8x8-unorm",
    "astc-8x8-unorm-srgb",
    "astc-10x5-unorm",
    "astc-10x5-unorm-srgb",
    "astc-10x6-unorm",
    "astc-10x6-unorm-srgb",
    "astc-10x8-unorm",
    "astc-10x8-unorm-srgb",
    "astc-10x10-unorm",
    "astc-10x10-unorm-srgb",
    "astc-12x10-unorm",
    "astc-12x10-unorm-srgb",
    "astc-12x12-unorm",
    "astc-12x12-unorm-srgb",
];

/// A device with every feature its adapter has; the feature names.
fn device_with_every_feature() -> (u32, Vec<String>) {
    let adapter = adapter();
    let features = WgpuAdapter::features(adapter).unwrap();
    let device = block(WgpuAdapter::request_device(
        adapter,
        json!({ "requiredFeatures": features }),
    ))
    .unwrap();
    (device, features)
}

#[test]
fn every_spec_format_makes_a_texture_where_its_feature_is_there() {
    let (device, features) = device_with_every_feature();
    let mut made = 0;
    for name in SPEC_FORMATS {
        let format: wgpu::TextureFormat = serde_json::from_value(json!(name)).unwrap();
        // The spec's feature for a format (wgpu's 16-bit norm one is
        // texture-formats-tier1).
        let needs = format.required_features();
        let has = needs.is_empty()
            || needs.iter().all(|f| {
                let spec = match f {
                    wgpu::Features::TEXTURE_FORMAT_16BIT_NORM => "texture-formats-tier1",
                    other => other.as_str().unwrap(),
                };
                features.iter().any(|n| n == spec)
            });
        let (block_w, block_h) = format.block_dimensions();
        let (texture, error) = validation_error(device, || {
            WgpuTexture::create(
                device,
                json!({ "size": [block_w * 2, block_h * 2], "format": name, "usage": TEXTURE_BINDING }),
            )
            .unwrap()
        });
        assert_eq!(error.is_none(), has, "{name}: {error:?}");
        if has {
            let entry = TextureEntry::lookup(texture).unwrap();
            assert_eq!(entry.texture.format(), format, "{name}");
            WgpuTextureView::create(texture, json!(null)).unwrap();
            made += 1;
        }
    }
    eprintln!(
        "{made} of {} spec formats on this adapter",
        SPEC_FORMATS.len()
    );
    assert!(made >= 60, "{made}");
    // What wgpu reads but WebGPU does not have is a TypeError.
    for name in ["r64uint", "nv12", "p010", "astc-4x4-hdr", "rgba8", ""] {
        assert_throws(
            WgpuTexture::create(
                device,
                json!({ "size": [4, 4], "format": name, "usage": TEXTURE_BINDING }),
            ),
            "TypeError",
        );
    }
}

/// `{ view descriptor, texture descriptor, valid }` cases of `createView`.
fn view_cases() -> Vec<(Value, Value, bool)> {
    let t2d = json!({ "size": [8, 8, 12], "format": "rgba8unorm", "usage": TEXTURE_BINDING, "mipLevelCount": 4 });
    let depth =
        json!({ "size": [8, 8], "format": "depth24plus-stencil8", "usage": TEXTURE_BINDING });
    let t3d = json!({ "size": [4, 4, 4], "dimension": "3d", "format": "rgba8unorm", "usage": TEXTURE_BINDING });
    let t1d = json!({ "size": [16], "dimension": "1d", "format": "rgba8unorm", "usage": TEXTURE_BINDING });
    let srgb = json!({ "size": [4, 4], "format": "rgba8unorm", "usage": TEXTURE_BINDING, "viewFormats": ["rgba8unorm-srgb"] });
    vec![
        (json!(null), t2d.clone(), true),
        (json!({ "dimension": "2d-array" }), t2d.clone(), true),
        (
            json!({ "dimension": "2d", "baseArrayLayer": 3, "arrayLayerCount": 1 }),
            t2d.clone(),
            true,
        ),
        (
            json!({ "dimension": "cube", "baseArrayLayer": 6, "arrayLayerCount": 6 }),
            t2d.clone(),
            true,
        ),
        (
            json!({ "dimension": "cube-array", "arrayLayerCount": 12 }),
            t2d.clone(),
            true,
        ),
        (
            json!({ "baseMipLevel": 1, "mipLevelCount": 2, "dimension": "2d-array" }),
            t2d.clone(),
            true,
        ),
        (
            json!({ "usage": TEXTURE_BINDING, "label": "sampled" }),
            t2d.clone(),
            true,
        ),
        (json!({ "baseMipLevel": 4 }), t2d.clone(), false),
        (
            json!({ "dimension": "cube", "arrayLayerCount": 5 }),
            t2d.clone(),
            false,
        ),
        (json!({ "usage": RENDER_ATTACHMENT }), t2d.clone(), false),
        (json!({ "usage": 0x100 }), t2d.clone(), false),
        (json!({ "format": "rgba8unorm-srgb" }), t2d.clone(), false),
        (json!({ "format": "rgba8unorm-srgb" }), srgb, true),
        (json!({ "aspect": "depth-only" }), depth.clone(), true),
        (json!({ "aspect": "stencil-only" }), depth.clone(), true),
        (json!({ "dimension": "3d" }), t3d.clone(), true),
        (json!({ "dimension": "2d" }), t3d, false),
        (json!({ "dimension": "1d" }), t1d, true),
    ]
}

#[test]
fn create_view_takes_every_descriptor_field() {
    let device = device();
    for (view, texture, valid) in view_cases() {
        let id = WgpuTexture::create(device, texture.clone()).unwrap();
        let (_, error) = validation_error(device, || {
            WgpuTextureView::create(id, view.clone()).unwrap()
        });
        assert_eq!(error.is_none(), valid, "{view} of {texture}: {error:?}");
    }
    let texture = WgpuTexture::create(
        device,
        json!({ "size": [4, 4], "format": "rgba8unorm", "usage": TEXTURE_BINDING }),
    )
    .unwrap();
    assert_throws(
        WgpuTextureView::create(texture, json!({ "aspect": "plane0" })),
        "TypeError",
    );
    assert_throws(
        WgpuTextureView::create(texture, json!({ "dimension": "4d" })),
        "TypeError",
    );
}

#[test]
fn texture_descriptors_dimensions_and_destroy() {
    let device = device();
    for (descriptor, valid) in [
        (
            json!({ "size": { "width": 16 }, "dimension": "1d", "format": "r8unorm", "usage": COPY_DST }),
            true,
        ),
        (
            json!({ "size": [16, 16, 16], "dimension": "3d", "format": "rgba8unorm", "usage": COPY_DST, "mipLevelCount": 5 }),
            true,
        ),
        (
            json!({ "size": [16, 16], "format": "rgba8unorm", "usage": RENDER_ATTACHMENT, "sampleCount": 4 }),
            true,
        ),
        (
            json!({ "size": [16, 16], "format": "rgba8unorm", "usage": COPY_DST, "mipLevelCount": 6 }),
            false,
        ),
        (
            json!({ "size": [16, 16], "format": "rgba8unorm", "usage": COPY_DST, "sampleCount": 3 }),
            false,
        ),
        (
            json!({ "size": [0, 16], "format": "rgba8unorm", "usage": COPY_DST }),
            false,
        ),
        (
            json!({ "size": [16, 16], "format": "rgba8unorm", "usage": 0 }),
            false,
        ),
        (
            json!({ "size": [16, 16], "format": "rgba8unorm", "usage": 0x40 }),
            false,
        ),
        (
            json!({ "size": [5, 5], "format": "bc1-rgba-unorm", "usage": COPY_DST }),
            false,
        ),
    ] {
        let (_, error) = validation_error(device, || {
            WgpuTexture::create(device, descriptor.clone()).unwrap()
        });
        assert_eq!(error.is_none(), valid, "{descriptor}: {error:?}");
    }
    let texture = WgpuTexture::create(
        device,
        json!({ "size": [4, 4], "format": "rgba8unorm", "usage": COPY_SRC, "label": "texture" }),
    )
    .unwrap();
    let entry = TextureEntry::lookup(texture).unwrap();
    assert_eq!(
        entry.texture.size(),
        wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1
        }
    );
    WgpuTexture::destroy(texture).unwrap();
    WgpuTexture::destroy(texture).unwrap();
    assert_throws(
        WgpuTexture::create(
            device,
            json!({ "size": [], "format": "rgba8unorm", "usage": 1 }),
        ),
        "TypeError",
    );
}

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

/// Texture.spec "Create texture and reads it": the triangle into the
/// reference's 1024 × 1024 canvas texture (the preferred format), read
/// back and compared with `snapshots/texture.png`.
#[test]
fn a_rendered_texture_reads_back_as_the_reference() {
    let device = device();
    let format = Wgpu::preferred_canvas_format();
    let size = 1024u32;
    let vertex = WgpuShaderModule::create(device, json!({ "code": TRIANGLE_VERT_WGSL })).unwrap();
    let fragment = WgpuShaderModule::create(device, json!({ "code": RED_FRAG_WGSL })).unwrap();
    let pipeline = WgpuRenderPipeline::create(
        device,
        json!({
            "layout": "auto",
            "vertex": { "module": vertex },
            "fragment": { "module": fragment, "targets": [{ "format": format }] },
            "primitive": { "topology": "triangle-list" },
        }),
    )
    .unwrap();
    let texture = WgpuTexture::create(
        device,
        json!({ "size": [size, size], "format": format, "usage": RENDER_ATTACHMENT | COPY_SRC }),
    )
    .unwrap();
    let view = WgpuTextureView::create(texture, json!(null)).unwrap();
    let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
    let pass = WgpuCommandEncoder::begin_render_pass(
        encoder,
        json!({ "colorAttachments": [{ "view": view, "clearValue": [0, 0, 0, 0], "loadOp": "clear", "storeOp": "store" }] }),
    )
    .unwrap();
    WgpuRenderPassEncoder::set_pipeline(pass, pipeline).unwrap();
    WgpuRenderPassEncoder::draw(pass, 3, None, None, None).unwrap();
    WgpuRenderPassEncoder::end(pass).unwrap();
    let readback = WgpuBuffer::create(
        device,
        json!({ "size": size * size * 4, "usage": 0x08 | 0x01 }),
    )
    .unwrap();
    WgpuCommandEncoder::copy_texture_to_buffer(
        encoder,
        json!({ "texture": texture }),
        json!({ "buffer": readback, "bytesPerRow": size * 4 }),
        json!([size, size]),
    )
    .unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
    wait(WgpuBuffer::map_async(readback, 1, None, None).unwrap()).unwrap();
    let mut pixels = WgpuBuffer::get_mapped_range(readback, None, None).unwrap();
    assert!(BufferEntry::lookup(readback).is_some());
    if format.starts_with("bgra") {
        for p in pixels.as_chunks_mut::<4>().0 {
            p.swap(0, 2);
        }
    }
    let image = image::RgbaImage::from_raw(size, size, pixels).unwrap();
    let differing = check_image(&image, "texture.png");
    eprintln!("texture.png: {differing} pixels differ");
}

/// Formats.spec: the texture-formats-tier1 formats, and the single
/// component and BGRA vertex formats.
#[test]
fn tier1_texture_formats_and_the_newer_vertex_formats() {
    let adapter = adapter();
    if WgpuAdapter::features(adapter)
        .unwrap()
        .contains(&"texture-formats-tier1".to_owned())
    {
        let device = block(WgpuAdapter::request_device(
            adapter,
            json!({ "requiredFeatures": ["texture-formats-tier1"] }),
        ))
        .unwrap();
        for format in [
            "r16unorm",
            "r16snorm",
            "rg16unorm",
            "rg16snorm",
            "rgba16unorm",
            "rgba16snorm",
        ] {
            let (texture, error) = validation_error(device, || {
                WgpuTexture::create(
                    device,
                    json!({ "size": [4, 4], "format": format, "usage": TEXTURE_BINDING | COPY_DST }),
                )
                .unwrap()
            });
            assert_eq!(error, None, "{format}");
            assert_eq!(
                serde_json::to_value(TextureEntry::lookup(texture).unwrap().texture.format())
                    .unwrap(),
                json!(format)
            );
        }
    } else {
        eprintln!("texture-formats-tier1 is not supported, skipping");
    }
    let device = device();
    let module = WgpuShaderModule::create(
        device,
        json!({ "code": "
          struct In {
            @location(0) bgra: vec4f,
            @location(1) u8: u32,
            @location(2) s8: i32,
            @location(3) un8: f32,
            @location(4) sn8: f32,
            @location(5) u16: u32,
            @location(6) s16: i32,
            @location(7) un16: f32,
            @location(8) sn16: f32,
            @location(9) f16: f32,
          }
          @vertex fn vs(in: In) -> @builtin(position) vec4f {
            let sum = in.bgra.x + f32(in.u8) + f32(in.s8) + in.un8 + in.sn8 +
              f32(in.u16) + f32(in.s16) + in.un16 + in.sn16 + in.f16;
            return vec4f(sum, 0, 0, 1);
          }
          @fragment fn fs() -> @location(0) vec4f { return vec4f(1); }
        " }),
    )
    .unwrap();
    let (_, error) = validation_error(device, || {
        WgpuRenderPipeline::create(
            device,
            json!({
                "layout": "auto",
                "vertex": { "module": module, "buffers": [{
                    "arrayStride": 20,
                    "attributes": [
                        { "shaderLocation": 0, "offset": 0, "format": "unorm8x4-bgra" },
                        { "shaderLocation": 1, "offset": 4, "format": "uint8" },
                        { "shaderLocation": 2, "offset": 5, "format": "sint8" },
                        { "shaderLocation": 3, "offset": 6, "format": "unorm8" },
                        { "shaderLocation": 4, "offset": 7, "format": "snorm8" },
                        { "shaderLocation": 5, "offset": 8, "format": "uint16" },
                        { "shaderLocation": 6, "offset": 10, "format": "sint16" },
                        { "shaderLocation": 7, "offset": 12, "format": "unorm16" },
                        { "shaderLocation": 8, "offset": 14, "format": "snorm16" },
                        { "shaderLocation": 9, "offset": 16, "format": "float16" },
                    ],
                }] },
                "fragment": { "module": module, "targets": [{ "format": "rgba8unorm" }] },
            }),
        )
        .unwrap()
    });
    assert_eq!(error, None);
}
