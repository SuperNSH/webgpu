//! `GPURenderBundleEncoder`: `device.createRenderBundleEncoder`. It records
//! the draw commands a render pass has (`render_pass_encoder.rs`
//! `DrawCommand`) and `finish()` replays them into a
//! `wgpu::RenderBundleEncoder`, which borrows every resource for its
//! lifetime and so cannot outlive one native call.
//!
//! wgpu panics, rather than reporting, on an invalid bundle encoder
//! descriptor or bundle (`Device::create_render_bundle_encoder`,
//! `RenderBundleEncoder::finish`); both run under `catch_unwind` here and
//! the panic becomes the spec's validation error and an invalid object.

use std::{
    num::NonZeroU32,
    panic::{AssertUnwindSafe, catch_unwind},
};

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::{
        command_encoder::label,
        device::DeviceEntry,
        render_bundle::RenderBundleEntry,
        render_pass_encoder::{
            self, DrawCommand, DrawTarget, Made, Recording, immediates, record, span,
        },
        render_pipeline::RenderPipelineEntry,
    },
    bytes::Bytes,
    convert::parse,
    error::{GpuErrorKind, NativeError, guard},
    registry::{self, Id},
};

/// `GPURenderBundleEncoderDescriptor`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBundleEncoderDescriptor {
    #[serde(default)]
    label: String,
    color_formats: Vec<Option<wgpu::TextureFormat>>,
    depth_stencil_format: Option<wgpu::TextureFormat>,
    #[serde(default = "one")]
    sample_count: u32,
    #[serde(default)]
    depth_read_only: bool,
    #[serde(default)]
    stencil_read_only: bool,
}

fn one() -> u32 {
    1
}

impl RenderBundleEncoderDescriptor {
    /// What the spec's "validating GPURenderPassLayout" finds wrong that
    /// wgpu's encoder does not check.
    fn problem(&self) -> Option<String> {
        if self.color_formats.iter().all(Option::is_none) && self.depth_stencil_format.is_none() {
            return Some("colorFormats has no format and there is no depthStencilFormat".into());
        }
        if let Some(f) = self
            .color_formats
            .iter()
            .flatten()
            .find(|f| f.is_depth_stencil_format() || f.is_compressed())
        {
            return Some(format!("{f:?} is not a color-renderable format"));
        }
        if let Some(f) = self.depth_stencil_format
            && !f.is_depth_stencil_format()
        {
            return Some(format!("{f:?} is not a depth or stencil format"));
        }
        if !matches!(self.sample_count, 1 | 4) {
            return Some(format!("sampleCount {} is not 1 or 4", self.sample_count));
        }
        None
    }

    /// Runs `f` with wgpu's descriptor.
    fn with<R>(&self, f: impl FnOnce(&wgpu::RenderBundleEncoderDescriptor<'_>) -> R) -> R {
        f(&wgpu::RenderBundleEncoderDescriptor {
            label: label(&self.label),
            color_formats: &self.color_formats,
            depth_stencil: self
                .depth_stencil_format
                .map(|format| wgpu::RenderBundleDepthStencil {
                    format,
                    depth_read_only: self.depth_read_only,
                    stencil_read_only: self.stencil_read_only,
                }),
            sample_count: self.sample_count,
            multiview: None::<NonZeroU32>,
        })
    }
}

/// A render bundle encoder in the registry.
pub struct RenderBundleEncoderEntry {
    /// Its device.
    pub device: Id,
    descriptor: RenderBundleEncoderDescriptor,
    commands: Vec<DrawCommand>,
    error: Option<String>,
    finished: bool,
}

impl Recording for RenderBundleEncoderEntry {
    type Command = DrawCommand;
    const KIND: &'static str = "render bundle encoder";
    fn device(&self) -> Id {
        self.device
    }
    fn ended(&self) -> bool {
        self.finished
    }
    fn commands(&mut self) -> &mut Vec<DrawCommand> {
        &mut self.commands
    }
    fn error(&mut self) -> &mut Option<String> {
        &mut self.error
    }
}

/// `f`'s result, or the message of its panic.
fn catch<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|panic| {
        panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_else(|| "invalid render bundle".into())
    })
}

fn rec(
    encoder: u32,
    what: &str,
    make: impl FnOnce() -> Result<Made<DrawCommand>, NativeError>,
) -> Result<(), String> {
    guard(|| record::<RenderBundleEncoderEntry>(encoder, what, make))
}

/// The native half of `GPURenderBundleEncoder`.
#[module]
pub struct WgpuRenderBundleEncoder;

#[module]
impl WgpuRenderBundleEncoder {
    /// `device.createRenderBundleEncoder(descriptor)`: the encoder's id. An
    /// invalid descriptor (no attachment, a format that cannot render) is
    /// a validation error, and the encoder is invalid.
    pub fn create(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let desc: RenderBundleEncoderDescriptor =
                parse("GPURenderBundleEncoderDescriptor", descriptor)?;
            // Validated now, as the spec does, by making (and dropping) one.
            let error = desc.problem().or_else(|| {
                desc.with(|d| catch(|| drop(entry.device.create_render_bundle_encoder(d))))
                    .err()
            });
            if let Some(e) = &error {
                entry.report(
                    GpuErrorKind::Validation,
                    format!("createRenderBundleEncoder: {e}"),
                );
            }
            Ok(registry::insert(RenderBundleEncoderEntry {
                device,
                descriptor: desc,
                commands: Vec::new(),
                // Reported already: finish() gives an invalid bundle silently.
                error: error.map(|_| String::new()),
                finished: false,
            }))
        })
    }

    /// `bundleEncoder.setPipeline(pipeline)`.
    pub fn set_pipeline(encoder: u32, pipeline: u32) -> Result<(), String> {
        rec(encoder, "setPipeline", || {
            let p = registry::get::<RenderPipelineEntry>(pipeline)?.pipeline;
            Ok(Ok(DrawCommand::SetPipeline(p)))
        })
    }

    /// `bundleEncoder.setBindGroup(index, bindGroup, dynamicOffsets)`.
    pub fn set_bind_group(
        encoder: u32,
        index: u32,
        bind_group: Option<u32>,
        dynamic_offsets: Vec<u32>,
    ) -> Result<(), String> {
        rec(encoder, "setBindGroup", || {
            render_pass_encoder::set_bind_group(index, bind_group, dynamic_offsets)
        })
    }

    /// `bundleEncoder.setVertexBuffer(slot, buffer, offset, size)`.
    pub fn set_vertex_buffer(
        encoder: u32,
        slot: u32,
        buffer: Option<u32>,
        offset: Option<f64>,
        size: Option<f64>,
    ) -> Result<(), String> {
        let device =
            guard(|| registry::with_mut::<RenderBundleEncoderEntry, _>(encoder, |e| e.device))?;
        rec(encoder, "setVertexBuffer", || {
            render_pass_encoder::set_vertex_buffer(
                device,
                slot,
                buffer,
                offset.unwrap_or(0.0),
                size,
            )
        })
    }

    /// `bundleEncoder.setIndexBuffer(buffer, indexFormat, offset, size)`.
    pub fn set_index_buffer(
        encoder: u32,
        buffer: u32,
        index_format: String,
        offset: Option<f64>,
        size: Option<f64>,
    ) -> Result<(), String> {
        rec(encoder, "setIndexBuffer", || {
            render_pass_encoder::set_index_buffer(buffer, index_format, offset.unwrap_or(0.0), size)
        })
    }

    /// `bundleEncoder.draw(vertexCount, instanceCount, firstVertex,
    /// firstInstance)`.
    pub fn draw(
        encoder: u32,
        vertex_count: u32,
        instance_count: Option<u32>,
        first_vertex: Option<u32>,
        first_instance: Option<u32>,
    ) -> Result<(), String> {
        rec(encoder, "draw", || {
            Ok(Ok(DrawCommand::Draw(
                span(first_vertex, vertex_count),
                span(first_instance, instance_count.unwrap_or(1)),
            )))
        })
    }

    /// `bundleEncoder.drawIndexed(indexCount, instanceCount, firstIndex,
    /// baseVertex, firstInstance)`.
    pub fn draw_indexed(
        encoder: u32,
        index_count: u32,
        instance_count: Option<u32>,
        first_index: Option<u32>,
        base_vertex: Option<i32>,
        first_instance: Option<u32>,
    ) -> Result<(), String> {
        rec(encoder, "drawIndexed", || {
            Ok(Ok(DrawCommand::DrawIndexed(
                span(first_index, index_count),
                base_vertex.unwrap_or(0),
                span(first_instance, instance_count.unwrap_or(1)),
            )))
        })
    }

    /// `bundleEncoder.drawIndirect(indirectBuffer, indirectOffset)`.
    pub fn draw_indirect(
        encoder: u32,
        indirect_buffer: u32,
        indirect_offset: f64,
    ) -> Result<(), String> {
        rec(encoder, "drawIndirect", || {
            render_pass_encoder::draw_indirect(false, indirect_buffer, indirect_offset)
        })
    }

    /// `bundleEncoder.drawIndexedIndirect(indirectBuffer, indirectOffset)`.
    pub fn draw_indexed_indirect(
        encoder: u32,
        indirect_buffer: u32,
        indirect_offset: f64,
    ) -> Result<(), String> {
        rec(encoder, "drawIndexedIndirect", || {
            render_pass_encoder::draw_indirect(true, indirect_buffer, indirect_offset)
        })
    }

    /// `bundleEncoder.setImmediates(rangeOffset, data)` with the bytes TSX
    /// cut.
    pub fn set_immediates(encoder: u32, range_offset: u32, data: Bytes) -> Result<(), String> {
        rec(encoder, "setImmediates", || {
            Ok(immediates(range_offset, data).map(|(o, d)| DrawCommand::SetImmediates(o, d)))
        })
    }

    /// `bundleEncoder.finish(descriptor)`: the bundle's id. Invalid
    /// recording is a validation error and gives an invalid bundle;
    /// finishing twice too.
    pub fn finish(encoder: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            #[derive(Deserialize, Default)]
            struct Desc {
                #[serde(default)]
                label: String,
            }
            let desc: Desc = parse("GPURenderBundleDescriptor", descriptor)?;
            let (device, finished, error, commands, bundle_desc) =
                registry::with_mut::<RenderBundleEncoderEntry, _>(encoder, |e| {
                    let finished = e.finished;
                    e.finished = true;
                    (
                        e.device,
                        finished,
                        e.error.take(),
                        std::mem::take(&mut e.commands),
                        e.descriptor.clone(),
                    )
                })?;
            let entry = DeviceEntry::get(device)?;
            let problem = if finished {
                Some("finish: the render bundle encoder is already finished".to_owned())
            } else {
                error.map(|e| {
                    if e.is_empty() {
                        e
                    } else {
                        format!("finish: {e}")
                    }
                })
            };
            let bundle = match problem {
                Some(problem) => {
                    if !problem.is_empty() {
                        entry.report(GpuErrorKind::Validation, problem);
                    }
                    None
                }
                None => {
                    let made = bundle_desc.with(|d| {
                        catch(|| {
                            let mut wgpu_encoder = entry.device.create_render_bundle_encoder(d);
                            for command in &commands {
                                wgpu_encoder.replay(command);
                            }
                            wgpu_encoder.finish(&wgpu::RenderBundleDescriptor {
                                label: label(&desc.label),
                            })
                        })
                    });
                    match made {
                        Ok(bundle) => Some(bundle),
                        Err(e) => {
                            entry.report(GpuErrorKind::Validation, format!("finish: {e}"));
                            None
                        }
                    }
                }
            };
            Ok(registry::insert(RenderBundleEntry { bundle, device }))
        })
    }
}
