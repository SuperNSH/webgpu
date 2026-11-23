//! `GPUCommandEncoder`: `device.createCommandEncoder`, its passes, copies,
//! clears, query resolves, debug groups and `finish()`.
//!
//! **Passes record, the encoder replays.** A wgpu pass borrows its encoder
//! mutably until it is dropped, and its resources for its lifetime; the JS
//! API hands out a pass object and lets the program call it whenever it
//! likes. So a pass here is a list of commands (`render_pass_encoder.rs`,
//! `compute_pass_encoder.rs`) holding owned wgpu handles (resolved from ids
//! when each call is made, so a JS object collected before `end()` changes
//! nothing). `end()` begins the wgpu pass on the encoder, replays the list
//! and drops the pass, all in one call. The encoder follows the spec's
//! state machine itself: "open", "locked" while a pass records, "ended"
//! after `finish()`; a call on a locked encoder invalidates it, a call on
//! an ended one is a validation error, and an invalid encoder's `finish()`
//! reports its first problem and gives an invalid command buffer. See
//! `docs/parity/commands.md`.

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;

use crate::{
    api::{
        buffer::BufferEntry, command_buffer::CommandBufferEntry, device::DeviceEntry,
        query_set::QuerySetEntry, render_pass_encoder::RenderPassEntry, texture::TextureEntry,
        texture_view::TextureViewEntry,
    },
    convert::{Color, Extent3d, TexelCopyBufferInfo, TexelCopyTextureInfo, parse},
    error::{GpuErrorKind, NativeError, guard},
    registry::{self, Id},
};

/// The spec's states of an encoder (`[[state]]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EncoderState {
    /// Commands may be encoded.
    Open,
    /// A pass is recording.
    Locked,
    /// `finish()` was called.
    Ended,
}

/// A command encoder in the registry.
pub struct CommandEncoderEntry {
    /// The encoder; `None` once finished.
    pub encoder: Option<wgpu::CommandEncoder>,
    /// Its device.
    pub device: Id,
    /// The spec's state.
    pub(crate) state: EncoderState,
    /// Why it is invalid (the first problem), if it is.
    pub(crate) invalid: Option<String>,
}

/// `name` as WebIDL `[EnforceRange] unsigned long long` converts it: a
/// `TypeError` for NaN, an infinity, a negative number or one past 2^53,
/// else truncated toward zero.
pub(crate) fn enforce_u64(name: &str, value: f64) -> Result<u64, NativeError> {
    const MAX_SAFE: f64 = 9_007_199_254_740_991.0;
    if !value.is_finite() || value.trunc() < 0.0 || value.trunc() > MAX_SAFE {
        return Err(NativeError::type_error(format!(
            "{name} ({value}) is outside the range of an unsigned long long"
        )));
    }
    Ok(value.trunc() as u64)
}

/// A label as wgpu takes it (`None` for `""`).
pub(crate) fn label(label: &str) -> Option<&str> {
    Some(label).filter(|l| !l.is_empty())
}

/// What a call on encoder `id` may do: run (open), nothing (it was
/// locked: it is invalid now), or nothing with a validation error (ended,
/// reported here). `what` names the call. Returns the device too.
fn admit(id: Id, what: &str) -> Result<(Id, bool), NativeError> {
    let (device, state) = registry::with_mut::<CommandEncoderEntry, _>(id, |e| {
        if e.state == EncoderState::Locked {
            e.invalid
                .get_or_insert_with(|| format!("{what} was called while a pass was open"));
        }
        (e.device, e.state)
    })?;
    if state == EncoderState::Ended {
        DeviceEntry::get(device)?.report(
            GpuErrorKind::Validation,
            format!("{what}: the command encoder is finished"),
        );
    }
    Ok((device, state == EncoderState::Open))
}

/// Runs `f` on encoder `id` when it is open (see [`admit`]).
pub(crate) fn with_encoder<R>(
    id: Id,
    what: &str,
    f: impl FnOnce(&mut wgpu::CommandEncoder) -> R,
) -> Result<Option<R>, NativeError> {
    let (_, open) = admit(id, what)?;
    if !open {
        return Ok(None);
    }
    registry::with_mut::<CommandEncoderEntry, _>(id, |e| e.encoder.as_mut().map(f))
}

/// Makes encoder `id` invalid with `reason` (its first problem stays).
pub(crate) fn invalidate(id: Id, reason: String) -> Result<(), NativeError> {
    registry::with_mut::<CommandEncoderEntry, _>(id, |e| {
        e.invalid.get_or_insert(reason);
    })
}

/// Begins a pass on encoder `id`: locks it. `Ok((device, valid))`, the
/// pass invalid when the encoder was not open.
pub(crate) fn lock(id: Id, what: &str) -> Result<(Id, bool), NativeError> {
    let (device, open) = admit(id, what)?;
    if open {
        registry::with_mut::<CommandEncoderEntry, _>(id, |e| e.state = EncoderState::Locked)?;
    }
    Ok((device, open))
}

/// Ends a pass of encoder `id`: unlocks it and, unless the pass is
/// invalid (`error`: the encoder becomes invalid), replays it with
/// `replay`. A pass whose encoder was finished meanwhile records nothing.
pub(crate) fn unlock(
    id: Id,
    error: Option<String>,
    replay: impl FnOnce(&mut wgpu::CommandEncoder),
) -> Result<(), NativeError> {
    registry::with_mut::<CommandEncoderEntry, _>(id, |e| {
        if e.state != EncoderState::Locked {
            return;
        }
        e.state = EncoderState::Open;
        if let Some(reason) = error {
            e.invalid.get_or_insert(reason);
            return;
        }
        if e.invalid.is_none()
            && let Some(encoder) = e.encoder.as_mut()
        {
            replay(encoder);
        }
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderDescriptor {
    #[serde(default)]
    label: String,
}

/// `GPULoadOp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum LoadOp {
    Load,
    Clear,
}

/// `GPURenderPassColorAttachment`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ColorAttachment {
    view: Id,
    depth_slice: Option<u32>,
    resolve_target: Option<Id>,
    clear_value: Option<Color>,
    load_op: LoadOp,
    store_op: wgpu::StoreOp,
}

/// `GPURenderPassDepthStencilAttachment`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DepthStencilAttachment {
    view: Id,
    depth_clear_value: Option<f32>,
    depth_load_op: Option<LoadOp>,
    depth_store_op: Option<wgpu::StoreOp>,
    #[serde(default)]
    depth_read_only: bool,
    #[serde(default)]
    stencil_clear_value: u32,
    stencil_load_op: Option<LoadOp>,
    stencil_store_op: Option<wgpu::StoreOp>,
    #[serde(default)]
    stencil_read_only: bool,
}

/// `GPURenderPassTimestampWrites` / `GPUComputePassTimestampWrites`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimestampWrites {
    query_set: Id,
    beginning_of_pass_write_index: Option<u32>,
    end_of_pass_write_index: Option<u32>,
}

/// Why query set `set` cannot be used by `what` (a validation error):
/// invalid, destroyed, or not of type `ty`.
pub(crate) fn query_set_problem(set: &QuerySetEntry, ty: &str, what: &str) -> Option<String> {
    let right_type = match ty {
        "occlusion" => matches!(set.ty, wgpu::QueryType::Occlusion),
        _ => matches!(set.ty, wgpu::QueryType::Timestamp),
    };
    if !set.valid {
        Some(format!("{what}: the query set is invalid"))
    } else if set.destroyed() {
        Some(format!("{what}: the query set is destroyed"))
    } else if !right_type {
        Some(format!("{what}: the query set is not of type \"{ty}\""))
    } else {
        None
    }
}

/// Timestamp writes with the query set resolved.
pub(crate) struct OwnedTimestampWrites {
    pub(crate) query_set: wgpu::QuerySet,
    pub(crate) beginning: Option<u32>,
    pub(crate) end: Option<u32>,
}

impl TimestampWrites {
    /// The writes with their set resolved, and why the set cannot be used.
    pub(crate) fn resolve(&self) -> Result<(OwnedTimestampWrites, Option<String>), NativeError> {
        let set = registry::get::<QuerySetEntry>(self.query_set)?;
        let problem = query_set_problem(&set, "timestamp", "timestampWrites");
        Ok((
            OwnedTimestampWrites {
                query_set: set.query_set,
                beginning: self.beginning_of_pass_write_index,
                end: self.end_of_pass_write_index,
            },
            problem,
        ))
    }
}

/// `GPURenderPassDescriptor`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassDescriptor {
    #[serde(default)]
    label: String,
    color_attachments: Vec<Option<ColorAttachment>>,
    depth_stencil_attachment: Option<DepthStencilAttachment>,
    occlusion_query_set: Option<Id>,
    timestamp_writes: Option<TimestampWrites>,
    /// Accepted; wgpu has no draw-count limit to check it against.
    #[serde(default)]
    #[allow(dead_code)]
    max_draw_count: Option<f64>,
}

/// `GPUComputePassDescriptor`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassDescriptor {
    #[serde(default)]
    label: String,
    timestamp_writes: Option<TimestampWrites>,
}

/// A color attachment with its views resolved.
pub(crate) struct OwnedColorAttachment {
    view: wgpu::TextureView,
    depth_slice: Option<u32>,
    resolve_target: Option<wgpu::TextureView>,
    ops: wgpu::Operations<wgpu::Color>,
}

/// A depth-stencil attachment with its view resolved.
pub(crate) struct OwnedDepthStencil {
    view: wgpu::TextureView,
    depth_ops: Option<wgpu::Operations<f32>>,
    stencil_ops: Option<wgpu::Operations<u32>>,
}

/// A render pass descriptor with every id resolved: what `end()` begins
/// the wgpu pass with.
pub(crate) struct OwnedRenderPass {
    label: String,
    colors: Vec<Option<OwnedColorAttachment>>,
    depth_stencil: Option<OwnedDepthStencil>,
    /// The occlusion query set (`beginOcclusionQuery` checks it again).
    pub(crate) occlusion: Option<QuerySetEntry>,
    timestamp_writes: Option<OwnedTimestampWrites>,
}

impl OwnedRenderPass {
    /// A placeholder left in an ended pass's entry.
    pub(crate) fn empty() -> Self {
        Self {
            label: String::new(),
            colors: Vec::new(),
            depth_stencil: None,
            occlusion: None,
            timestamp_writes: None,
        }
    }

    /// Begins the wgpu pass on `encoder` and runs `f` in it.
    pub(crate) fn run(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        f: impl FnOnce(&mut wgpu::RenderPass<'_>),
    ) {
        let colors: Vec<Option<wgpu::RenderPassColorAttachment<'_>>> = self
            .colors
            .iter()
            .map(|c| {
                c.as_ref().map(|c| wgpu::RenderPassColorAttachment {
                    view: &c.view,
                    depth_slice: c.depth_slice,
                    resolve_target: c.resolve_target.as_ref(),
                    ops: c.ops,
                })
            })
            .collect();
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: label(&self.label),
            color_attachments: &colors,
            depth_stencil_attachment: self.depth_stencil.as_ref().map(|d| {
                wgpu::RenderPassDepthStencilAttachment {
                    view: &d.view,
                    depth_ops: d.depth_ops,
                    stencil_ops: d.stencil_ops,
                }
            }),
            timestamp_writes: self.timestamp_writes.as_ref().map(|t| {
                wgpu::RenderPassTimestampWrites {
                    query_set: &t.query_set,
                    beginning_of_pass_write_index: t.beginning,
                    end_of_pass_write_index: t.end,
                }
            }),
            occlusion_query_set: self.occlusion.as_ref().map(|o| &o.query_set),
            multiview_mask: None,
        });
        f(&mut pass);
    }
}

/// A compute pass descriptor with its query set resolved.
pub(crate) struct OwnedComputePass {
    label: String,
    timestamp_writes: Option<OwnedTimestampWrites>,
}

impl OwnedComputePass {
    /// Begins the wgpu pass on `encoder` and runs `f` in it.
    pub(crate) fn run(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        f: impl FnOnce(&mut wgpu::ComputePass<'_>),
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: label(&self.label),
            timestamp_writes: self.timestamp_writes.as_ref().map(|t| {
                wgpu::ComputePassTimestampWrites {
                    query_set: &t.query_set,
                    beginning_of_pass_write_index: t.beginning,
                    end_of_pass_write_index: t.end,
                }
            }),
        });
        f(&mut pass);
    }
}

fn view(id: Id) -> Result<wgpu::TextureView, NativeError> {
    Ok(registry::get::<TextureViewEntry>(id)?.view)
}

/// One aspect's ops of a depth-stencil attachment, as the spec validates
/// them: none for a read-only aspect or one the format lacks (both ops
/// must be absent then), both for the others. `Err` is why the pass is
/// invalid.
fn aspect_ops<V: Copy>(
    name: &str,
    has_aspect: bool,
    read_only: bool,
    load: Option<LoadOp>,
    store: Option<wgpu::StoreOp>,
    clear: Option<V>,
) -> Result<Option<wgpu::Operations<V>>, String> {
    if read_only || !has_aspect {
        if load.is_some() || store.is_some() {
            return Err(format!(
                "{name}LoadOp and {name}StoreOp must be absent for a read-only or missing {name} aspect"
            ));
        }
        return Ok(None);
    }
    let (Some(load), Some(store)) = (load, store) else {
        return Err(format!(
            "{name}LoadOp and {name}StoreOp are required for the {name} aspect"
        ));
    };
    let load = match load {
        LoadOp::Load => wgpu::LoadOp::Load,
        LoadOp::Clear => match clear {
            Some(v) => wgpu::LoadOp::Clear(v),
            None => return Err(format!("{name}ClearValue is required to clear")),
        },
    };
    Ok(Some(wgpu::Operations { load, store }))
}

impl RenderPassDescriptor {
    /// The descriptor with its ids resolved; `Ok(Err(reason))` when the
    /// spec makes the pass invalid.
    fn resolve(self) -> Result<(OwnedRenderPass, Option<String>), NativeError> {
        let mut error = None;
        let mut colors = Vec::with_capacity(self.color_attachments.len());
        for a in self.color_attachments {
            colors.push(match a {
                None => None,
                Some(a) => Some(OwnedColorAttachment {
                    view: view(a.view)?,
                    depth_slice: a.depth_slice,
                    resolve_target: a.resolve_target.map(view).transpose()?,
                    ops: wgpu::Operations {
                        load: match a.load_op {
                            LoadOp::Clear => wgpu::LoadOp::Clear(
                                a.clear_value
                                    .as_ref()
                                    .map(Color::to_wgpu)
                                    .transpose()?
                                    .unwrap_or(wgpu::Color::TRANSPARENT),
                            ),
                            LoadOp::Load => wgpu::LoadOp::Load,
                        },
                        store: a.store_op,
                    },
                }),
            });
        }
        let depth_stencil = match self.depth_stencil_attachment {
            None => None,
            Some(d) => {
                let view = view(d.view)?;
                let format = view.texture().format();
                if let Some(v) = d.depth_clear_value
                    && !(0.0..=1.0).contains(&v)
                {
                    error.get_or_insert(format!("depthClearValue {v} is outside [0, 1]"));
                }
                let depth_ops = aspect_ops(
                    "depth",
                    format.has_depth_aspect(),
                    d.depth_read_only,
                    d.depth_load_op,
                    d.depth_store_op,
                    d.depth_clear_value,
                );
                let stencil_ops = aspect_ops(
                    "stencil",
                    format.has_stencil_aspect(),
                    d.stencil_read_only,
                    d.stencil_load_op,
                    d.stencil_store_op,
                    Some(d.stencil_clear_value),
                );
                let depth_ops = depth_ops.unwrap_or_else(|e| {
                    error.get_or_insert(e);
                    None
                });
                let stencil_ops = stencil_ops.unwrap_or_else(|e| {
                    error.get_or_insert(e);
                    None
                });
                Some(OwnedDepthStencil {
                    view,
                    depth_ops,
                    stencil_ops,
                })
            }
        };
        let occlusion = self
            .occlusion_query_set
            .map(registry::get::<QuerySetEntry>)
            .transpose()?;
        if let Some(problem) = occlusion
            .as_ref()
            .and_then(|o| query_set_problem(o, "occlusion", "occlusionQuerySet"))
        {
            error.get_or_insert(problem);
        }
        let timestamp_writes = match self.timestamp_writes.as_ref().map(TimestampWrites::resolve) {
            None => None,
            Some(r) => {
                let (writes, problem) = r?;
                if let Some(problem) = problem {
                    error.get_or_insert(problem);
                }
                Some(writes)
            }
        };
        Ok((
            OwnedRenderPass {
                label: self.label,
                colors,
                depth_stencil,
                occlusion,
                timestamp_writes,
            },
            error,
        ))
    }
}

fn buffer(id: Id) -> Result<wgpu::Buffer, NativeError> {
    Ok(registry::get::<BufferEntry>(id)?.buffer)
}

fn texture(id: Id) -> Result<wgpu::Texture, NativeError> {
    Ok(registry::get::<TextureEntry>(id)?.texture)
}

/// A `GPUTexelCopyTextureInfo` as wgpu's, borrowing `texture`.
fn texture_info<'t>(
    info: &TexelCopyTextureInfo,
    texture: &'t wgpu::Texture,
) -> Result<wgpu::TexelCopyTextureInfo<'t>, NativeError> {
    Ok(wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: info.mip_level,
        origin: info.origin.to_wgpu()?,
        aspect: info.aspect,
    })
}

/// The native half of `GPUCommandEncoder`.
#[module]
pub struct WgpuCommandEncoder;

#[module]
impl WgpuCommandEncoder {
    /// `device.createCommandEncoder(descriptor)`: the encoder's id.
    pub fn create(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let desc: CommandEncoderDescriptor = parse("GPUCommandEncoderDescriptor", descriptor)?;
            let encoder = entry
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: label(&desc.label),
                });
            Ok(registry::insert(CommandEncoderEntry {
                encoder: Some(encoder),
                device,
                state: EncoderState::Open,
                invalid: None,
            }))
        })
    }

    /// `encoder.beginRenderPass(descriptor)`: the pass's id. The encoder
    /// is locked until the pass ends.
    pub fn begin_render_pass(encoder: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let desc: RenderPassDescriptor = parse("GPURenderPassDescriptor", descriptor)?;
            let (target, invalid) = desc.resolve()?;
            let (device, valid) = lock(encoder, "beginRenderPass")?;
            let error = if valid {
                invalid
            } else {
                Some("the command encoder was not open when the pass began".to_owned())
            };
            Ok(registry::insert(RenderPassEntry::new(
                device, encoder, target, error,
            )))
        })
    }

    /// `encoder.beginComputePass(descriptor)`: the pass's id. The encoder
    /// is locked until the pass ends.
    pub fn begin_compute_pass(encoder: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let desc: ComputePassDescriptor = parse("GPUComputePassDescriptor", descriptor)?;
            let (timestamp_writes, problem) =
                match desc.timestamp_writes.as_ref().map(TimestampWrites::resolve) {
                    None => (None, None),
                    Some(r) => {
                        let (w, p) = r?;
                        (Some(w), p)
                    }
                };
            let target = OwnedComputePass {
                label: desc.label,
                timestamp_writes,
            };
            let (device, valid) = lock(encoder, "beginComputePass")?;
            let error = if valid {
                problem
            } else {
                Some("the command encoder was not open when the pass began".to_owned())
            };
            Ok(registry::insert(
                crate::api::compute_pass_encoder::ComputePassEntry::new(
                    device, encoder, target, error,
                ),
            ))
        })
    }

    /// `encoder.copyBufferToBuffer(source, sourceOffset, destination,
    /// destinationOffset, size)`; `size` `null` copies to the source's end.
    /// Offsets and size are `[EnforceRange]` (a `TypeError` when negative);
    /// their alignment (4 bytes) and range are validated by wgpu and make
    /// the encoder invalid.
    pub fn copy_buffer_to_buffer(
        encoder: u32,
        source: u32,
        source_offset: f64,
        destination: u32,
        destination_offset: f64,
        size: Option<f64>,
    ) -> Result<(), String> {
        guard(|| {
            let source_offset = enforce_u64("sourceOffset", source_offset)?;
            let destination_offset = enforce_u64("destinationOffset", destination_offset)?;
            let size = size.map(|s| enforce_u64("size", s)).transpose()?;
            let src = buffer(source)?;
            let dst = buffer(destination)?;
            if let Some(reason) =
                buffer_copy_problem(&src, source_offset, &dst, destination_offset, size)
            {
                return invalidate_open(encoder, "copyBufferToBuffer", reason);
            }
            with_encoder(encoder, "copyBufferToBuffer", |e| {
                e.copy_buffer_to_buffer(&src, source_offset, &dst, destination_offset, size)
            })?;
            Ok(())
        })
    }

    /// `encoder.copyBufferToTexture(source, destination, copySize)`.
    pub fn copy_buffer_to_texture(
        encoder: u32,
        source: JsonValue,
        destination: JsonValue,
        copy_size: JsonValue,
    ) -> Result<(), String> {
        guard(|| {
            let source: TexelCopyBufferInfo = parse("GPUTexelCopyBufferInfo", source)?;
            let destination: TexelCopyTextureInfo = parse("GPUTexelCopyTextureInfo", destination)?;
            let size = parse::<Extent3d>("GPUExtent3D", copy_size)?.to_wgpu()?;
            let buf = buffer(source.buffer)?;
            let tex = texture(destination.texture)?;
            if let Some(reason) = row_alignment_problem(&source.layout) {
                return invalidate_open(encoder, "copyBufferToTexture", reason);
            }
            let dst = texture_info(&destination, &tex)?;
            with_encoder(encoder, "copyBufferToTexture", |e| {
                e.copy_buffer_to_texture(
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buf,
                        layout: source.layout.to_wgpu(),
                    },
                    dst,
                    size,
                )
            })?;
            Ok(())
        })
    }

    /// `encoder.copyTextureToBuffer(source, destination, copySize)`.
    pub fn copy_texture_to_buffer(
        encoder: u32,
        source: JsonValue,
        destination: JsonValue,
        copy_size: JsonValue,
    ) -> Result<(), String> {
        guard(|| {
            let source: TexelCopyTextureInfo = parse("GPUTexelCopyTextureInfo", source)?;
            let destination: TexelCopyBufferInfo = parse("GPUTexelCopyBufferInfo", destination)?;
            let size = parse::<Extent3d>("GPUExtent3D", copy_size)?.to_wgpu()?;
            let tex = texture(source.texture)?;
            let buf = buffer(destination.buffer)?;
            if let Some(reason) = row_alignment_problem(&destination.layout) {
                return invalidate_open(encoder, "copyTextureToBuffer", reason);
            }
            let src = texture_info(&source, &tex)?;
            with_encoder(encoder, "copyTextureToBuffer", |e| {
                e.copy_texture_to_buffer(
                    src,
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buf,
                        layout: destination.layout.to_wgpu(),
                    },
                    size,
                )
            })?;
            Ok(())
        })
    }

    /// `encoder.copyTextureToTexture(source, destination, copySize)`.
    pub fn copy_texture_to_texture(
        encoder: u32,
        source: JsonValue,
        destination: JsonValue,
        copy_size: JsonValue,
    ) -> Result<(), String> {
        guard(|| {
            let source: TexelCopyTextureInfo = parse("GPUTexelCopyTextureInfo", source)?;
            let destination: TexelCopyTextureInfo = parse("GPUTexelCopyTextureInfo", destination)?;
            let size = parse::<Extent3d>("GPUExtent3D", copy_size)?.to_wgpu()?;
            let src_tex = texture(source.texture)?;
            let dst_tex = texture(destination.texture)?;
            let src = texture_info(&source, &src_tex)?;
            let dst = texture_info(&destination, &dst_tex)?;
            with_encoder(encoder, "copyTextureToTexture", |e| {
                e.copy_texture_to_texture(src, dst, size)
            })?;
            Ok(())
        })
    }

    /// `encoder.clearBuffer(buffer, offset, size)`: zeroes the range
    /// (`size` `null`: to the end). Both must be multiples of 4.
    pub fn clear_buffer(
        encoder: u32,
        buffer_id: u32,
        offset: Option<f64>,
        size: Option<f64>,
    ) -> Result<(), String> {
        guard(|| {
            let offset = enforce_u64("offset", offset.unwrap_or(0.0))?;
            let size = size.map(|s| enforce_u64("size", s)).transpose()?;
            let buf = buffer(buffer_id)?;
            let end = offset.checked_add(size.unwrap_or(buf.size().saturating_sub(offset)));
            if !offset.is_multiple_of(4)
                || !size.unwrap_or(0).is_multiple_of(4)
                || end.is_none_or(|end| end > buf.size())
            {
                return invalidate_open(
                    encoder,
                    "clearBuffer",
                    format!(
                        "clearBuffer: offset {offset} and size {size:?} must be multiples of 4 inside {} bytes",
                        buf.size()
                    ),
                );
            }
            with_encoder(encoder, "clearBuffer", |e| {
                e.clear_buffer(&buf, offset, size)
            })?;
            Ok(())
        })
    }

    /// `encoder.resolveQuerySet(querySet, firstQuery, queryCount,
    /// destination, destinationOffset)`.
    pub fn resolve_query_set(
        encoder: u32,
        query_set: u32,
        first_query: u32,
        query_count: u32,
        destination: u32,
        destination_offset: f64,
    ) -> Result<(), String> {
        guard(|| {
            let destination_offset = enforce_u64("destinationOffset", destination_offset)?;
            let set_entry = registry::get::<QuerySetEntry>(query_set)?;
            let dst = buffer(destination)?;
            if let Some(problem) = query_set_problem(
                &set_entry,
                if matches!(set_entry.ty, wgpu::QueryType::Occlusion) {
                    "occlusion"
                } else {
                    "timestamp"
                },
                "resolveQuerySet",
            ) {
                return invalidate_open(encoder, "resolveQuerySet", problem);
            }
            let set = set_entry.query_set;
            let Some(end) = first_query.checked_add(query_count) else {
                return invalidate_open(
                    encoder,
                    "resolveQuerySet",
                    "resolveQuerySet: firstQuery + queryCount overflows".into(),
                );
            };
            if !destination_offset.is_multiple_of(256) {
                return invalidate_open(
                    encoder,
                    "resolveQuerySet",
                    format!(
                        "resolveQuerySet: destinationOffset {destination_offset} is not a multiple of 256"
                    ),
                );
            }
            with_encoder(encoder, "resolveQuerySet", |e| {
                e.resolve_query_set(&set, first_query..end, &dst, destination_offset)
            })?;
            Ok(())
        })
    }

    /// `encoder.pushDebugGroup(groupLabel)`.
    pub fn push_debug_group(encoder: u32, group_label: String) -> Result<(), String> {
        guard(|| {
            with_encoder(encoder, "pushDebugGroup", |e| {
                e.push_debug_group(&group_label)
            })?;
            Ok(())
        })
    }

    /// `encoder.popDebugGroup()`.
    pub fn pop_debug_group(encoder: u32) -> Result<(), String> {
        guard(|| {
            with_encoder(encoder, "popDebugGroup", |e| e.pop_debug_group())?;
            Ok(())
        })
    }

    /// `encoder.insertDebugMarker(markerLabel)`.
    pub fn insert_debug_marker(encoder: u32, marker_label: String) -> Result<(), String> {
        guard(|| {
            with_encoder(encoder, "insertDebugMarker", |e| {
                e.insert_debug_marker(&marker_label)
            })?;
            Ok(())
        })
    }

    /// `encoder.finish(descriptor)`: the command buffer's id. An invalid
    /// encoder (a pass left open, a bad pass or copy) reports a validation
    /// error and gives an invalid command buffer, as does finishing twice.
    pub fn finish(encoder: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let _: CommandEncoderDescriptor = parse("GPUCommandBufferDescriptor", descriptor)?;
            let (device, state, invalid, finished) =
                registry::with_mut::<CommandEncoderEntry, _>(encoder, |e| {
                    let state = e.state;
                    e.state = EncoderState::Ended;
                    (e.device, state, e.invalid.take(), e.encoder.take())
                })?;
            let problem = match state {
                EncoderState::Ended => Some("the command encoder is already finished".to_owned()),
                EncoderState::Locked => Some("a pass is still open".to_owned()),
                EncoderState::Open => invalid,
            };
            let buffer = match (problem, finished) {
                (None, Some(e)) => Some(e.finish()),
                (problem, _) => {
                    DeviceEntry::get(device)?.report(
                        GpuErrorKind::Validation,
                        format!(
                            "finish: {}",
                            problem.unwrap_or_else(|| "the command encoder is invalid".into())
                        ),
                    );
                    None
                }
            };
            Ok(registry::insert(CommandBufferEntry { buffer, device }))
        })
    }
}

/// Makes encoder `id` invalid for `reason` when it is open (a locked or
/// ended encoder gets what [`admit`] gives it instead).
fn invalidate_open(id: Id, what: &str, reason: String) -> Result<(), NativeError> {
    let (_, open) = admit(id, what)?;
    if open {
        invalidate(id, reason)?;
    }
    Ok(())
}

/// Why a `copyBufferToBuffer` is invalid where wgpu would panic or
/// mis-report: alignment and range, as the spec checks them.
fn buffer_copy_problem(
    src: &wgpu::Buffer,
    src_offset: u64,
    dst: &wgpu::Buffer,
    dst_offset: u64,
    size: Option<u64>,
) -> Option<String> {
    let size = size.unwrap_or_else(|| src.size().saturating_sub(src_offset));
    if !src_offset.is_multiple_of(4) || !dst_offset.is_multiple_of(4) || !size.is_multiple_of(4) {
        return Some(format!(
            "copyBufferToBuffer: sourceOffset {src_offset}, destinationOffset {dst_offset} and size {size} must be multiples of 4"
        ));
    }
    if src_offset
        .checked_add(size)
        .is_none_or(|end| end > src.size())
        || dst_offset
            .checked_add(size)
            .is_none_or(|end| end > dst.size())
    {
        return Some(format!(
            "copyBufferToBuffer: {size} bytes from {src_offset} to {dst_offset} do not fit the buffers ({} and {} bytes)",
            src.size(),
            dst.size()
        ));
    }
    None
}

/// Why a buffer-texture copy's layout is invalid: `bytesPerRow` must be a
/// multiple of 256 (`COPY_BYTES_PER_ROW_ALIGNMENT`).
fn row_alignment_problem(layout: &crate::convert::TexelCopyBufferLayout) -> Option<String> {
    layout
        .bytes_per_row
        .filter(|b| !b.is_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
        .map(|b| {
            format!(
                "bytesPerRow {b} is not a multiple of {}",
                wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
            )
        })
}
