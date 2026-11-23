//! The canvas example's own crate: `CanvasDemo.draw(contextId, frame)`
//! renders a triangle over a clear color that cycles with `frame` into the
//! `webgpu.canvas` view of context `contextId`, straight on the
//! `wgpu::Surface` the platform view attached (`nsh_webgpu::surface::views`)
//! or, on Linux, through the offscreen target its picture shows
//! (`nsh_webgpu::surface::offscreen`). It stands in for `GPUCanvasContext`
//! (area surfaces-apple) so the platform views can be seen on screen on
//! their own.

use std::sync::{Mutex, OnceLock};

use nsh_core::prelude::*;
use nsh_webgpu::surface::views;

#[native_module("canvas/native")]
fn native(registry: &mut Registry) {
    registry.module::<CanvasDemo>();
}

const SHADER: &str = "
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  var pos = array<vec2f, 3>(vec2(0.0, 0.6), vec2(-0.6, -0.6), vec2(0.6, -0.6));
  return vec4f(pos[i], 0.0, 1.0);
}
@fragment
fn fs() -> @location(0) vec4f {
  return vec4f(1.0, 0.0, 0.0, 1.0);
}";

/// The GPU objects the demo draws with.
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: wgpu::Adapter,
    name: String,
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    configured: std::collections::BTreeMap<u32, (u32, u32)>,
}

static GPU: OnceLock<Mutex<Option<Gpu>>> = OnceLock::new();

fn gpu(surface: &wgpu::Surface<'_>) -> Result<std::sync::MutexGuard<'static, Option<Gpu>>, String> {
    let mut guard = GPU
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let instance = nsh_webgpu::instance::instance();
        // The adapters that can present to the surface, Vulkan / DX12 / Metal
        // before GL (what wgpu's default `request_adapter` does not promise).
        let all = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
        let listed: Vec<String> = all
            .iter()
            .map(|a| {
                let i = a.get_info();
                format!(
                    "{} {:?} presents={}",
                    i.name,
                    i.backend,
                    a.is_surface_supported(surface)
                )
            })
            .collect();
        log::warn!("canvas demo adapters: {listed:?}");
        let mut adapters: Vec<wgpu::Adapter> = all
            .into_iter()
            .filter(|a| a.is_surface_supported(surface))
            .collect();
        adapters.sort_by_key(|a| a.get_info().backend == wgpu::Backend::Gl);
        let adapter = adapters
            .into_iter()
            .next()
            .ok_or("no adapter can present to the surface")?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits:
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| format!("no device: {e}"))?;
        *guard = Some(Gpu {
            device,
            queue,
            adapter,
            name: format!("{} ({:?})", info.name, info.backend),
            pipelines: Vec::new(),
            configured: Default::default(),
        });
    }
    Ok(guard)
}

fn pipeline(gpu: &mut Gpu, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    if let Some((_, p)) = gpu.pipelines.iter().find(|(f, _)| *f == format) {
        return p.clone();
    }
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
    let p = gpu
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
    gpu.pipelines.push((format, p.clone()));
    p
}

fn clear(frame: u32) -> wgpu::Color {
    let t = f64::from(frame % 120) / 120.0;
    wgpu::Color {
        r: 0.1,
        g: 0.2 + 0.5 * t,
        b: 0.7 - 0.5 * t,
        a: 1.0,
    }
}

fn encode(gpu: &Gpu, view: &wgpu::TextureView, pipeline: &wgpu::RenderPipeline, frame: u32) {
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear(frame)),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.draw(0..3, 0..1);
    }
    gpu.queue.submit([encoder.finish()]);
}

/// Draws the demo into a canvas view.
#[module]
pub struct CanvasDemo;

#[module]
impl CanvasDemo {
    /// Draws frame `frame` into context `context`'s view; what happened, as
    /// text for the screen (`"waiting"` until the view attached a surface).
    pub fn draw(context: u32, frame: u32) -> Result<String, String> {
        let Some(view) = views::get(context) else {
            return Ok(format!("context {context}: waiting for the surface"));
        };
        let mut guard = gpu(&view.surface)?;
        let gpu = guard.as_mut().ok_or("no GPU")?;
        let caps = view.surface.get_capabilities(&gpu.adapter);
        let name = gpu.name.clone();
        if caps.formats.is_empty() {
            return Ok(format!(
                "context {context}: {name} cannot present to the surface"
            ));
        }
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or(caps.formats.first().copied())
            .ok_or("the surface has no formats")?;
        let size = (view.width, view.height);
        if gpu.configured.get(&context) != Some(&size) {
            log::warn!("canvas demo: configuring {context} {size:?} {format:?} on {name}");
            view.surface.configure(
                &gpu.device,
                &wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format,
                    width: view.width,
                    height: view.height,
                    present_mode: wgpu::PresentMode::Fifo,
                    desired_maximum_frame_latency: 2,
                    alpha_mode: caps.alpha_modes[0],
                    view_formats: vec![],
                },
            );
            gpu.configured.insert(context, size);
        }
        let texture = match view.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                gpu.configured.remove(&context);
                return Ok(format!("context {context}: no texture this frame"));
            }
        };
        let pipeline = pipeline(gpu, format);
        let target = texture.texture.create_view(&Default::default());
        encode(gpu, &target, &pipeline, frame);
        texture.present();
        Ok(format!(
            "context {context}: {}x{} {format:?} on {}, frame {frame}",
            view.width, view.height, gpu.name
        ))
    }
}
