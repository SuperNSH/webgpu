//! What the tests share: a runtime, a device, the reference's offscreen
//! canvas (1024 x 1024 in the preferred format), readback, snapshots.

use std::{path::PathBuf, sync::OnceLock};

use nsh_webgpu::api::{
    adapter::WgpuAdapter, buffer::WgpuBuffer, command_encoder::WgpuCommandEncoder,
    device::WgpuDevice, gpu::Wgpu, queue::WgpuQueue, texture::WgpuTexture,
    texture_view::WgpuTextureView,
};
use serde_json::{Value, json};

use crate::pixelmatch;

/// The runtime the async functions run on.
pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .expect("a runtime")
    })
}

/// Runs `future` to its end.
pub(crate) fn block<F: Future>(future: F) -> F::Output {
    runtime().block_on(future)
}

/// A new device.
pub(crate) fn device() -> u32 {
    let adapter = block(Wgpu::request_adapter(json!({})))
        .expect("requestAdapter")
        .expect("an adapter on this machine");
    block(WgpuAdapter::request_device(adapter, json!({}))).expect("requestDevice")
}

/// Waits for ticket `ticket`.
pub(crate) fn wait(ticket: u32) -> Result<(), String> {
    block(Wgpu::wait(ticket))
}

/// `GPUBufferUsage` / `GPUTextureUsage` bits.
pub(crate) mod usage {
    pub(crate) const MAP_READ: u32 = 0x1;
    pub(crate) const COPY_SRC: u32 = 0x4;
    pub(crate) const COPY_DST: u32 = 0x8;
    pub(crate) const INDEX: u32 = 0x10;
    pub(crate) const VERTEX: u32 = 0x20;
    pub(crate) const STORAGE: u32 = 0x80;
    pub(crate) const INDIRECT: u32 = 0x100;
    pub(crate) const T_COPY_SRC: u32 = 0x1;
    pub(crate) const T_COPY_DST: u32 = 0x2;
    pub(crate) const T_TEXTURE_BINDING: u32 = 0x4;
    pub(crate) const T_RENDER_ATTACHMENT: u32 = 0x10;
}

/// A buffer of `size` bytes holding `bytes` (COPY_DST added).
pub(crate) fn buffer_with(device: u32, usage: u32, bytes: &[u8]) -> u32 {
    let buffer = WgpuBuffer::create(
        device,
        json!({ "size": bytes.len(), "usage": usage | usage::COPY_DST }),
    )
    .unwrap();
    WgpuQueue::write_buffer(
        device,
        buffer,
        0.0,
        nsh_webgpu::bytes::Bytes(bytes.to_vec()),
    )
    .unwrap();
    buffer
}

/// `f32`s as bytes.
pub(crate) fn f32s(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// `u32`s as bytes.
pub(crate) fn u32s(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// The contents of `buffer` (`MAP_READ`), mapped and copied out.
pub(crate) fn read_buffer(buffer: u32) -> Vec<u8> {
    let ticket = WgpuBuffer::map_async(buffer, 1, None, None).unwrap();
    wait(ticket).unwrap();
    let bytes = WgpuBuffer::get_mapped_range(buffer, None, None).unwrap();
    WgpuBuffer::unmap(buffer).unwrap();
    bytes
}

/// `buffer`'s bytes after copying them to a mappable buffer.
pub(crate) fn read_back(device: u32, buffer: u32, size: u64) -> Vec<u8> {
    let out = WgpuBuffer::create(
        device,
        json!({ "size": size, "usage": usage::COPY_DST | usage::MAP_READ }),
    )
    .unwrap();
    let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
    WgpuCommandEncoder::copy_buffer_to_buffer(encoder, buffer, 0.0, out, 0.0, Some(size as f64))
        .unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
    read_buffer(out)
}

/// Texture `texture`'s `width` x `height` texels of `bpp` bytes, rows
/// packed (copied through a buffer with 256-byte rows).
pub(crate) fn read_texture(
    device: u32,
    texture: u32,
    width: u32,
    height: u32,
    bpp: u32,
) -> Vec<u8> {
    let row = (width * bpp).div_ceil(256) * 256;
    let out = WgpuBuffer::create(
        device,
        json!({ "size": row * height, "usage": usage::COPY_DST | usage::MAP_READ }),
    )
    .unwrap();
    let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
    WgpuCommandEncoder::copy_texture_to_buffer(
        encoder,
        json!({ "texture": texture }),
        json!({ "buffer": out, "bytesPerRow": row }),
        json!([width, height]),
    )
    .unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
    let mapped = read_buffer(out);
    (0..height as usize)
        .flat_map(|y| mapped[y * row as usize..y * row as usize + (width * bpp) as usize].to_vec())
        .collect()
}

/// The reference's offscreen canvas: a texture of the preferred format.
pub(crate) struct Canvas {
    pub(crate) device: u32,
    pub(crate) texture: u32,
    pub(crate) format: String,
    pub(crate) size: u32,
}

impl Canvas {
    /// A 1024 x 1024 canvas, as the reference's harness has.
    pub(crate) fn new(device: u32) -> Self {
        Self::sized(device, 1024)
    }

    /// A `size` x `size` canvas.
    pub(crate) fn sized(device: u32, size: u32) -> Self {
        let format = Wgpu::preferred_canvas_format();
        let texture = WgpuTexture::create(
            device,
            json!({
                "size": [size, size],
                "format": format,
                "usage": usage::T_RENDER_ATTACHMENT | usage::T_COPY_SRC | usage::T_COPY_DST,
            }),
        )
        .unwrap();
        Self {
            device,
            texture,
            format,
            size,
        }
    }

    /// A view of the current texture (`ctx.getCurrentTexture().createView()`).
    pub(crate) fn view(&self) -> u32 {
        WgpuTextureView::create(self.texture, json!(null)).unwrap()
    }

    /// A color attachment that clears the canvas to `clear`.
    pub(crate) fn attachment(&self, clear: [f64; 4]) -> Value {
        json!({ "view": self.view(), "clearValue": clear, "loadOp": "clear", "storeOp": "store" })
    }

    /// The pixels as RGBA rows (`canvas.getImageData()`, then the
    /// harness's `encodeImage` swizzle).
    pub(crate) fn rgba(&self) -> Vec<u8> {
        let mut pixels = read_texture(self.device, self.texture, self.size, self.size, 4);
        if self.format.starts_with("bgra") {
            for p in pixels.as_chunks_mut::<4>().0 {
                p.swap(0, 2);
            }
        }
        pixels
    }
}

/// `examples/headless/snapshots/commands/<name>`.
pub(crate) fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples/headless/snapshots/commands")
        .join(name)
}

/// The bytes of snapshot or asset `name`.
pub(crate) fn asset(name: &str) -> Vec<u8> {
    std::fs::read(snapshot_path(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Saves `rgba` (`width` x `height`) as `<name>` in `$NSH_WEBGPU_EVIDENCE`
/// when it is set.
pub(crate) fn evidence(name: &str, rgba: &[u8], width: u32, height: u32) {
    if let Ok(dir) = std::env::var("NSH_WEBGPU_EVIDENCE") {
        let image = image::RgbaImage::from_raw(width, height, rgba.to_vec()).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        image
            .save(PathBuf::from(dir).join(name.replace('/', "-")))
            .unwrap();
    }
}

/// Checks `rgba` against snapshot `name` as the reference's `checkImage`
/// does (pixelmatch threshold 0.1, at most 200 different pixels). Saves
/// the image as evidence.
pub(crate) fn check_image(rgba: &[u8], width: u32, height: u32, name: &str) {
    check_image_with(rgba, width, height, name, 200);
}

/// [`check_image`] with its own pixel budget.
pub(crate) fn check_image_with(rgba: &[u8], width: u32, height: u32, name: &str, max_diff: usize) {
    let expected = image::load_from_memory(&asset(name)).unwrap().to_rgba8();
    evidence(name, rgba, width, height);
    assert_eq!(
        expected.dimensions(),
        (width, height),
        "{name}: the image sizes differ"
    );
    let diff = pixelmatch::count(expected.as_raw(), rgba, width, height, 0.1);
    assert!(
        diff <= max_diff,
        "{name}: {diff} pixels differ (at most {max_diff})"
    );
}

/// Pops the device's innermost error scope: the message, if it caught one.
pub(crate) fn pop_error(device: u32) -> Option<String> {
    let e = WgpuDevice::pop_error_scope(device).unwrap();
    e["message"].as_str().map(str::to_owned)
}

/// The reference's `triangle.ts`.
pub(crate) const TRIANGLE_VERT_WGSL: &str = "@vertex
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

/// The reference's red fragment shader.
pub(crate) const RED_FRAG_WGSL: &str = "@fragment
fn main() -> @location(0) vec4f {
  return vec4(1.0, 0.0, 0.0, 1.0);
}";

/// A render pipeline of `vertex`/`fragment` WGSL (entry points by
/// default) into `format`, with `extra` members merged in.
pub(crate) fn pipeline(
    device: u32,
    vertex: &str,
    fragment: &str,
    format: &str,
    extra: Value,
) -> u32 {
    use nsh_webgpu::api::{render_pipeline::WgpuRenderPipeline, shader_module::WgpuShaderModule};
    let vs = WgpuShaderModule::create(device, json!({ "code": vertex })).unwrap();
    let fs = if fragment == vertex {
        vs
    } else {
        WgpuShaderModule::create(device, json!({ "code": fragment })).unwrap()
    };
    let mut desc = json!({
        "layout": "auto",
        "vertex": { "module": vs },
        "fragment": { "module": fs, "targets": [{ "format": format }] },
        "primitive": { "topology": "triangle-list" },
    });
    if let (Some(d), Some(e)) = (desc.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            match (d.get_mut(k), v) {
                (Some(Value::Object(dst)), Value::Object(src)) => {
                    for (k2, v2) in src {
                        dst.insert(k2.clone(), v2.clone());
                    }
                }
                _ => {
                    d.insert(k.clone(), v.clone());
                }
            }
        }
    }
    WgpuRenderPipeline::create(device, desc).unwrap()
}
