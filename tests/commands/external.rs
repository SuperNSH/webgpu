//! `ExternalTexture.spec.ts`: an image through `createImageBitmap` and
//! `copyExternalImageToTexture` (flipY true, omitted, false), sampled on a
//! quad, against the reference's snapshots. The spec's fourth case imports
//! a native video frame (react-native-webgpu's `NativeBuffer`, area (e)).
//! `importExternalTexture` from a `GPUTexture` is checked in
//! `src/api/external_texture.rs`'s unit tests, and bound in a bind group
//! and sampled in a render below (`an_imported_texture_binds_and_renders`).

use nsh_webgpu::{
    api::{
        bind_group::WgpuBindGroup, command_encoder::WgpuCommandEncoder, device::WgpuDevice,
        external_texture::WgpuExternalTexture, image_bitmap::WgpuImageBitmap, queue::WgpuQueue,
        render_pass_encoder::WgpuRenderPassEncoder as Pass, render_pipeline::WgpuRenderPipeline,
        sampler::WgpuSampler, texture::WgpuTexture, texture_view::WgpuTextureView,
    },
    bytes::Bytes,
};
use serde_json::{Value, json};

use crate::common::*;

const QUAD: &str = "struct OurVertexShaderOutput {
  @builtin(position) position: vec4f,
  @location(0) texcoord: vec2f,
};
@vertex fn vs(@builtin(vertex_index) vertexIndex : u32) -> OurVertexShaderOutput {
  let pos = array(
    vec2f( 0.0,  0.0), vec2f( 1.0,  0.0), vec2f( 0.0,  1.0),
    vec2f( 0.0,  1.0), vec2f( 1.0,  0.0), vec2f( 1.0,  1.0),
  );
  var vsOutput: OurVertexShaderOutput;
  let xy = pos[vertexIndex];
  vsOutput.position = vec4f(xy, 0.0, 1.0);
  vsOutput.texcoord = xy;
  return vsOutput;
}
@group(0) @binding(0) var ourSampler: sampler;
@group(0) @binding(1) var ourTexture: texture_2d<f32>;
@fragment fn fs(fsInput: OurVertexShaderOutput) -> @location(0) vec4f {
  return textureSample(ourTexture, ourSampler, fsInput.texcoord);
}";

/// The spec's scene with `source` (the `flipY` member, or none).
fn render_f(flip_y: Value) -> Vec<u8> {
    let device = device();
    let canvas = Canvas::new(device);
    let p = pipeline(device, QUAD, QUAD, &canvas.format, json!({}));
    let bitmap = block(WgpuImageBitmap::decode(
        Bytes(asset("assets/f.png")),
        json!({ "colorSpaceConversion": "none" }),
    ))
    .unwrap();
    let (w, h) = (bitmap["width"].clone(), bitmap["height"].clone());
    let texture = WgpuTexture::create(
        device,
        json!({ "format": "rgba8unorm", "size": [w, h],
            "usage": usage::T_TEXTURE_BINDING | usage::T_COPY_DST | usage::T_RENDER_ATTACHMENT }),
    )
    .unwrap();
    let mut source = json!({ "bitmap": bitmap["id"] });
    if !flip_y.is_null() {
        source["flipY"] = flip_y;
    }
    WgpuQueue::copy_external_image_to_texture(
        device,
        source,
        json!({ "texture": texture }),
        json!({ "width": w, "height": h }),
    )
    .unwrap();
    let sampler = WgpuSampler::create(
        device,
        json!({ "addressModeU": "repeat", "addressModeV": "repeat", "magFilter": "linear" }),
    )
    .unwrap();
    let layout = WgpuRenderPipeline::get_bind_group_layout(p, 0).unwrap();
    let group = WgpuBindGroup::create(
        device,
        json!({ "layout": layout, "entries": [
            { "binding": 0, "resource": sampler },
            { "binding": 1, "resource": WgpuTextureView::create(texture, json!(null)).unwrap() },
        ] }),
    )
    .unwrap();
    let encoder =
        WgpuCommandEncoder::create(device, json!({ "label": "render quad encoder" })).unwrap();
    let pass = WgpuCommandEncoder::begin_render_pass(
        encoder,
        json!({ "label": "our basic canvas renderPass", "colorAttachments": [canvas.attachment([0.3, 0.3, 0.3, 1.0])] }),
    )
    .unwrap();
    Pass::set_pipeline(pass, p).unwrap();
    Pass::set_bind_group(pass, 0, Some(group), vec![]).unwrap();
    Pass::draw(pass, 6, None, None, None).unwrap();
    Pass::end(pass).unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
    canvas.rgba()
}

#[test]
fn simple_1_flip_y() {
    check_image(&render_f(json!(true)), 1024, 1024, "f.png");
}

#[test]
fn simple_2_no_flip_y() {
    check_image(&render_f(json!(null)), 1024, 1024, "f2.png");
}

#[test]
fn flip_y_false_does_not_flip() {
    check_image(&render_f(json!(false)), 1024, 1024, "f2.png");
}

#[test]
fn importing_a_texture_needs_the_device_feature() {
    let device = device();
    let texture = WgpuTexture::create(
        device,
        json!({ "size": [4, 4], "format": "rgba8unorm", "usage": usage::T_TEXTURE_BINDING }),
    )
    .unwrap();
    if WgpuExternalTexture::supported(device).unwrap() {
        let id = WgpuExternalTexture::import(device, json!({ "source": texture, "rotation": 90 }))
            .unwrap();
        WgpuExternalTexture::destroy(id).unwrap();
        WgpuExternalTexture::destroy(id).unwrap();
        let e = WgpuExternalTexture::import(device, json!({ "source": texture, "rotation": 45 }))
            .unwrap_err();
        assert!(e.starts_with("TypeError:"), "{e}");
        let e = WgpuExternalTexture::import(
            device,
            json!({ "source": texture, "cropOrigin": [2, 2], "cropSize": [4, 4] }),
        )
        .unwrap_err();
        assert!(e.starts_with("OperationError:"), "{e}");
    } else {
        let e = WgpuExternalTexture::import(device, json!({ "source": texture })).unwrap_err();
        assert!(e.starts_with("ERR_WEBGPU_UNSUPPORTED:"), "{e}");
    }
}

/// A full-canvas triangle whose fragment loads the external texture at its
/// own pixel: the canvas shows the external texture's pixels as they are.
const LOAD_EXTERNAL: &str = "@group(0) @binding(0) var frame: texture_external;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  var pos = array<vec2f, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
  return vec4f(pos[i], 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
  return textureLoad(frame, vec2u(p.xy));
}";

/// Renders the 4 x 4 `rgba` (RGBA8 rows) through an external texture
/// imported with `options` merged into the descriptor; the canvas's RGBA,
/// or `None` when the device cannot import one.
fn render_external(rgba: &[u8], options: Value) -> Option<Vec<u8>> {
    let device = device();
    if !WgpuExternalTexture::supported(device).unwrap() {
        return None;
    }
    WgpuDevice::push_error_scope(device, "validation".into()).unwrap();
    let texture = WgpuTexture::create(
        device,
        json!({ "size": [4, 4], "format": "rgba8unorm",
            "usage": usage::T_TEXTURE_BINDING | usage::T_COPY_DST }),
    )
    .unwrap();
    WgpuQueue::write_texture(
        device,
        json!({ "texture": texture }),
        Bytes(rgba.to_vec()),
        json!({ "bytesPerRow": 16 }),
        json!([4, 4]),
    )
    .unwrap();
    let mut descriptor = json!({ "source": texture });
    if let (Some(d), Some(o)) = (descriptor.as_object_mut(), options.as_object()) {
        d.extend(o.clone());
    }
    let external = WgpuExternalTexture::import(device, descriptor).unwrap();
    let canvas = Canvas::sized(device, 4);
    let p = pipeline(
        device,
        LOAD_EXTERNAL,
        LOAD_EXTERNAL,
        &canvas.format,
        json!({}),
    );
    let layout = WgpuRenderPipeline::get_bind_group_layout(p, 0).unwrap();
    let group = WgpuBindGroup::create(
        device,
        json!({ "layout": layout, "entries": [{ "binding": 0, "resource": external }] }),
    )
    .unwrap();
    let encoder = WgpuCommandEncoder::create(device, json!({})).unwrap();
    let pass = WgpuCommandEncoder::begin_render_pass(
        encoder,
        json!({ "colorAttachments": [canvas.attachment([0.0, 0.0, 0.0, 0.0])] }),
    )
    .unwrap();
    Pass::set_pipeline(pass, p).unwrap();
    Pass::set_bind_group(pass, 0, Some(group), vec![]).unwrap();
    Pass::draw(pass, 3, None, None, None).unwrap();
    Pass::end(pass).unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
    assert_eq!(pop_error(device), None, "a GPU error");
    Some(canvas.rgba())
}

#[test]
fn an_imported_texture_binds_and_renders() {
    // Each pixel its own color: x in red, y in green.
    let rgba: Vec<u8> = (0..16)
        .flat_map(|i: u8| [(i % 4) * 60, (i / 4) * 60, 200, 255])
        .collect();
    let Some(plain) = render_external(&rgba, json!({})) else {
        eprintln!("skipped: no EXTERNAL_TEXTURE on this adapter");
        return;
    };
    assert_eq!(plain, rgba, "the external texture's pixels, as they are");
    let mirrored = render_external(&rgba, json!({ "mirrored": true })).unwrap();
    let flipped: Vec<u8> = rgba
        .as_chunks::<16>()
        .0
        .iter()
        .flat_map(|row| {
            row.as_chunks::<4>()
                .0
                .iter()
                .rev()
                .flatten()
                .copied()
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(mirrored, flipped, "mirrored: each row reversed");
}
