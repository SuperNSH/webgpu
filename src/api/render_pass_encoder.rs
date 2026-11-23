//! `GPURenderPassEncoder`: records its commands (with owned wgpu handles)
//! until `end()`, which replays them into a wgpu render pass on the
//! encoder (`command_encoder.rs` explains why). The draw commands are
//! shared with render bundles (`render_bundle_encoder.rs`), which replay
//! the same list into a `wgpu::RenderBundleEncoder`.

use std::ops::Range;

use nsh_core::{JsonValue, prelude::*};

use crate::{
    api::{
        bind_group::BindGroupEntry,
        buffer::BufferEntry,
        command_encoder::{self, OwnedRenderPass},
        device::DeviceEntry,
        render_bundle::RenderBundleEntry,
        render_pipeline::RenderPipelineEntry,
    },
    bytes::Bytes,
    convert::{Color, parse},
    error::{GpuErrorKind, NativeError, guard},
    registry::{self, Id},
};

/// A command a render pass and a render bundle both record
/// (`GPURenderCommandsMixin`, `GPUBindingCommandsMixin`).
pub(crate) enum DrawCommand {
    SetPipeline(wgpu::RenderPipeline),
    SetBindGroup(u32, Option<wgpu::BindGroup>, Vec<u32>),
    /// A slot and its buffer range (`None` only from an invalid pass).
    SetVertexBuffer(u32, Option<(wgpu::Buffer, Range<u64>)>),
    SetIndexBuffer(wgpu::Buffer, wgpu::IndexFormat, Range<u64>),
    Draw(Range<u32>, Range<u32>),
    DrawIndexed(Range<u32>, i32, Range<u32>),
    DrawIndirect(wgpu::Buffer, u64),
    DrawIndexedIndirect(wgpu::Buffer, u64),
    SetImmediates(u32, Vec<u8>),
}

/// What a render pass records.
pub(crate) enum RenderCommand {
    Draw(DrawCommand),
    SetViewport([f32; 6]),
    SetScissorRect([u32; 4]),
    SetBlendConstant(wgpu::Color),
    SetStencilReference(u32),
    BeginOcclusionQuery(u32),
    EndOcclusionQuery,
    ExecuteBundles(Vec<wgpu::RenderBundle>),
    PushDebugGroup(String),
    PopDebugGroup,
    InsertDebugMarker(String),
}

/// Where draw commands replay: a render pass or a render bundle encoder.
pub(crate) trait DrawTarget<'a> {
    /// Replays `command`, borrowing its handles for `'a`.
    fn replay(&mut self, command: &'a DrawCommand);
}

macro_rules! draw_target {
    ($ty:ty) => {
        impl<'a> DrawTarget<'a> for $ty {
            fn replay(&mut self, command: &'a DrawCommand) {
                match command {
                    DrawCommand::SetPipeline(p) => self.set_pipeline(p),
                    DrawCommand::SetBindGroup(i, g, offsets) => {
                        self.set_bind_group(*i, g.as_ref(), offsets)
                    }
                    DrawCommand::SetVertexBuffer(slot, Some((b, range))) => {
                        self.set_vertex_buffer(*slot, b.slice(range.clone()))
                    }
                    DrawCommand::SetVertexBuffer(_, None) => {}
                    DrawCommand::SetIndexBuffer(b, format, range) => {
                        self.set_index_buffer(b.slice(range.clone()), *format)
                    }
                    DrawCommand::Draw(v, i) => self.draw(v.clone(), i.clone()),
                    DrawCommand::DrawIndexed(idx, base, i) => {
                        self.draw_indexed(idx.clone(), *base, i.clone())
                    }
                    DrawCommand::DrawIndirect(b, o) => self.draw_indirect(b, *o),
                    DrawCommand::DrawIndexedIndirect(b, o) => self.draw_indexed_indirect(b, *o),
                    DrawCommand::SetImmediates(o, data) => self.set_immediates(*o, data),
                }
            }
        }
    };
}

draw_target!(wgpu::RenderPass<'_>);
draw_target!(wgpu::RenderBundleEncoder<'a>);

/// Replays a render pass's commands.
fn replay(pass: &mut wgpu::RenderPass<'_>, commands: &[RenderCommand]) {
    for command in commands {
        match command {
            RenderCommand::Draw(c) => pass.replay(c),
            RenderCommand::SetViewport([x, y, w, h, min, max]) => {
                pass.set_viewport(*x, *y, *w, *h, *min, *max)
            }
            RenderCommand::SetScissorRect([x, y, w, h]) => pass.set_scissor_rect(*x, *y, *w, *h),
            RenderCommand::SetBlendConstant(c) => pass.set_blend_constant(*c),
            RenderCommand::SetStencilReference(r) => pass.set_stencil_reference(*r),
            RenderCommand::BeginOcclusionQuery(i) => pass.begin_occlusion_query(*i),
            RenderCommand::EndOcclusionQuery => pass.end_occlusion_query(),
            RenderCommand::ExecuteBundles(b) => pass.execute_bundles(b.iter()),
            RenderCommand::PushDebugGroup(l) => pass.push_debug_group(l),
            RenderCommand::PopDebugGroup => pass.pop_debug_group(),
            RenderCommand::InsertDebugMarker(l) => pass.insert_debug_marker(l),
        }
    }
}

/// A render pass in the registry.
pub struct RenderPassEntry {
    /// Its device.
    pub device: Id,
    /// Its encoder.
    pub(crate) encoder: Id,
    /// What the wgpu pass begins with.
    pub(crate) target: OwnedRenderPass,
    /// What it recorded.
    pub(crate) commands: Vec<RenderCommand>,
    /// Why it is invalid (its first problem), if it is.
    pub(crate) error: Option<String>,
    /// Whether `end()` was called.
    pub ended: bool,
}

impl RenderPassEntry {
    pub(crate) fn new(
        device: Id,
        encoder: Id,
        target: OwnedRenderPass,
        error: Option<String>,
    ) -> Self {
        Self {
            device,
            encoder,
            target,
            commands: Vec::new(),
            error,
            ended: false,
        }
    }
}

/// Something a pass or bundle encoder records into: commands, a first
/// error, whether it ended.
pub(crate) trait Recording: 'static + Send {
    /// The command type.
    type Command;
    /// The kind, for messages ("render pass").
    const KIND: &'static str;
    /// Its device.
    fn device(&self) -> Id;
    /// Whether it has ended.
    fn ended(&self) -> bool;
    /// Its commands.
    fn commands(&mut self) -> &mut Vec<Self::Command>;
    /// Its first error.
    fn error(&mut self) -> &mut Option<String>;
}

impl Recording for RenderPassEntry {
    type Command = RenderCommand;
    const KIND: &'static str = "render pass";
    fn device(&self) -> Id {
        self.device
    }
    fn ended(&self) -> bool {
        self.ended
    }
    fn commands(&mut self) -> &mut Vec<RenderCommand> {
        &mut self.commands
    }
    fn error(&mut self) -> &mut Option<String> {
        &mut self.error
    }
}

/// What a call on a recorder makes: a command, or why the recorder is
/// invalid now.
pub(crate) type Made<C> = Result<C, String>;

/// Records the command `make` makes into recorder `id` (a `T`): after an
/// end, the call is a validation error (`what` names it); a problem makes
/// the recorder invalid.
pub(crate) fn record<T: Recording>(
    id: Id,
    what: &str,
    make: impl FnOnce() -> Result<Made<T::Command>, NativeError>,
) -> Result<(), NativeError> {
    let made = make()?;
    let (device, ended) = registry::with_mut::<T, _>(id, |r| {
        if !r.ended() {
            match made {
                Ok(c) => r.commands().push(c),
                Err(e) => {
                    r.error().get_or_insert(format!("{what}: {e}"));
                }
            }
        }
        (r.device(), r.ended())
    })?;
    if ended {
        DeviceEntry::get(device)?.report(
            GpuErrorKind::Validation,
            format!("{what}: the {} has ended", T::KIND),
        );
    }
    Ok(())
}

/// A buffer range for a vertex or index buffer, as the spec validates it:
/// `size` `None` is to the end. `Ok(None)` is an empty range (nothing to
/// bind: wgpu has no empty slice).
pub(crate) fn buffer_range(
    buffer: &wgpu::Buffer,
    offset: u64,
    size: Option<u64>,
) -> Made<Option<Range<u64>>> {
    let whole = buffer.size();
    let size = match size {
        Some(s) => s,
        None if offset <= whole => whole - offset,
        None => {
            return Err(format!(
                "offset {offset} is past the buffer's {whole} bytes"
            ));
        }
    };
    match offset.checked_add(size) {
        Some(end) if end <= whole => Ok((size > 0).then_some(offset..end)),
        _ => Err(format!(
            "[{offset}, {offset} + {size}) is outside the buffer's {whole} bytes"
        )),
    }
}

/// `setBindGroup`'s command: the group (`null` unbinds) and its offsets.
pub(crate) fn set_bind_group(
    index: u32,
    bind_group: Option<u32>,
    dynamic_offsets: Vec<u32>,
) -> Result<Made<DrawCommand>, NativeError> {
    let group = bind_group
        .map(registry::get::<BindGroupEntry>)
        .transpose()?;
    if group.as_ref().is_some_and(|g| !g.valid) {
        return Ok(Err("the bind group is invalid".into()));
    }
    Ok(Ok(DrawCommand::SetBindGroup(
        index,
        group.map(|g| g.bind_group),
        dynamic_offsets,
    )))
}

/// `setVertexBuffer`'s command. `null` (or an empty range) unbinds the
/// slot: wgpu has no empty binding, so a 4-byte zeroed buffer of `device`
/// stands in. A slot a pipeline reads from then fails wgpu's size check
/// for any real draw, as an unbound slot fails the spec's.
pub(crate) fn set_vertex_buffer(
    device: Id,
    slot: u32,
    buffer: Option<u32>,
    offset: f64,
    size: Option<f64>,
) -> Result<Made<DrawCommand>, NativeError> {
    let offset = command_encoder::enforce_u64("offset", offset)?;
    let size = size
        .map(|s| command_encoder::enforce_u64("size", s))
        .transpose()?;
    let placeholder = || -> Result<Made<DrawCommand>, NativeError> {
        let dummy = DeviceEntry::get(device)?
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("nsh-webgpu: unbound vertex buffer slot"),
                size: 4,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });
        Ok(Ok(DrawCommand::SetVertexBuffer(slot, Some((dummy, 0..4)))))
    };
    let Some(id) = buffer else {
        return placeholder();
    };
    let buffer = registry::get::<BufferEntry>(id)?.buffer;
    if !offset.is_multiple_of(4) {
        return Ok(Err(format!("offset {offset} is not a multiple of 4")));
    }
    match buffer_range(&buffer, offset, size) {
        Ok(Some(range)) => Ok(Ok(DrawCommand::SetVertexBuffer(
            slot,
            Some((buffer, range)),
        ))),
        Ok(None) => placeholder(),
        Err(e) => Ok(Err(e)),
    }
}

/// `setIndexBuffer`'s command.
pub(crate) fn set_index_buffer(
    buffer: u32,
    format: String,
    offset: f64,
    size: Option<f64>,
) -> Result<Made<DrawCommand>, NativeError> {
    let format: wgpu::IndexFormat = parse("GPUIndexFormat", JsonValue::String(format))?;
    let offset = command_encoder::enforce_u64("offset", offset)?;
    let size = size
        .map(|s| command_encoder::enforce_u64("size", s))
        .transpose()?;
    let buffer = registry::get::<BufferEntry>(buffer)?.buffer;
    let element = match format {
        wgpu::IndexFormat::Uint16 => 2,
        wgpu::IndexFormat::Uint32 => 4,
    };
    if !offset.is_multiple_of(element) {
        return Ok(Err(format!(
            "offset {offset} is not a multiple of the index size {element}"
        )));
    }
    Ok(
        buffer_range(&buffer, offset, size).and_then(|range| match range {
            Some(range) => Ok(DrawCommand::SetIndexBuffer(buffer, format, range)),
            None => Err("an empty index buffer range cannot be bound in wgpu".into()),
        }),
    )
}

/// An indirect draw's command.
pub(crate) fn draw_indirect(
    indexed: bool,
    buffer: u32,
    offset: f64,
) -> Result<Made<DrawCommand>, NativeError> {
    let offset = command_encoder::enforce_u64("indirectOffset", offset)?;
    let buffer = registry::get::<BufferEntry>(buffer)?.buffer;
    let args = if indexed { 20 } else { 16 };
    if !offset.is_multiple_of(4) || offset.checked_add(args).is_none_or(|e| e > buffer.size()) {
        return Ok(Err(format!(
            "indirectOffset {offset} must be a multiple of 4 with {args} bytes of arguments inside the buffer's {} bytes",
            buffer.size()
        )));
    }
    Ok(Ok(if indexed {
        DrawCommand::DrawIndexedIndirect(buffer, offset)
    } else {
        DrawCommand::DrawIndirect(buffer, offset)
    }))
}

/// `[first, first + count)`, saturating (wgpu validates the range).
pub(crate) fn span(first: Option<u32>, count: u32) -> Range<u32> {
    let first = first.unwrap_or(0);
    first..first.saturating_add(count)
}

/// `setImmediates`' offset and bytes (TSX cut them from `data`).
pub(crate) fn immediates(offset: u32, data: Bytes) -> Made<(u32, Vec<u8>)> {
    if !offset.is_multiple_of(4) || !data.0.len().is_multiple_of(4) {
        return Err(format!(
            "rangeOffset {offset} and the {} bytes of data must be multiples of 4",
            data.0.len()
        ));
    }
    Ok((offset, data.0))
}

fn rec(
    pass: u32,
    what: &str,
    make: impl FnOnce() -> Result<Made<RenderCommand>, NativeError>,
) -> Result<(), String> {
    guard(|| record::<RenderPassEntry>(pass, what, make))
}

fn draw(
    pass: u32,
    what: &str,
    make: impl FnOnce() -> Result<Made<DrawCommand>, NativeError>,
) -> Result<(), String> {
    rec(pass, what, || Ok(make()?.map(RenderCommand::Draw)))
}

/// The native half of `GPURenderPassEncoder`.
#[module]
pub struct WgpuRenderPassEncoder;

#[module]
impl WgpuRenderPassEncoder {
    /// `pass.setPipeline(pipeline)`.
    pub fn set_pipeline(pass: u32, pipeline: u32) -> Result<(), String> {
        draw(pass, "setPipeline", || {
            let p = registry::get::<RenderPipelineEntry>(pipeline)?.pipeline;
            Ok(Ok(DrawCommand::SetPipeline(p)))
        })
    }

    /// `pass.setBindGroup(index, bindGroup, dynamicOffsets)` (TSX cuts the
    /// `Uint32Array, start, length` form to the offsets).
    pub fn set_bind_group(
        pass: u32,
        index: u32,
        bind_group: Option<u32>,
        dynamic_offsets: Vec<u32>,
    ) -> Result<(), String> {
        draw(pass, "setBindGroup", || {
            set_bind_group(index, bind_group, dynamic_offsets)
        })
    }

    /// `pass.setVertexBuffer(slot, buffer, offset, size)`; `buffer` `null`
    /// unbinds the slot.
    pub fn set_vertex_buffer(
        pass: u32,
        slot: u32,
        buffer: Option<u32>,
        offset: Option<f64>,
        size: Option<f64>,
    ) -> Result<(), String> {
        let device = guard(|| registry::with_mut::<RenderPassEntry, _>(pass, |p| p.device))?;
        draw(pass, "setVertexBuffer", || {
            set_vertex_buffer(device, slot, buffer, offset.unwrap_or(0.0), size)
        })
    }

    /// `pass.setIndexBuffer(buffer, indexFormat, offset, size)`.
    pub fn set_index_buffer(
        pass: u32,
        buffer: u32,
        index_format: String,
        offset: Option<f64>,
        size: Option<f64>,
    ) -> Result<(), String> {
        draw(pass, "setIndexBuffer", || {
            set_index_buffer(buffer, index_format, offset.unwrap_or(0.0), size)
        })
    }

    /// `pass.draw(vertexCount, instanceCount, firstVertex, firstInstance)`.
    pub fn draw(
        pass: u32,
        vertex_count: u32,
        instance_count: Option<u32>,
        first_vertex: Option<u32>,
        first_instance: Option<u32>,
    ) -> Result<(), String> {
        draw(pass, "draw", || {
            Ok(Ok(DrawCommand::Draw(
                span(first_vertex, vertex_count),
                span(first_instance, instance_count.unwrap_or(1)),
            )))
        })
    }

    /// `pass.drawIndexed(indexCount, instanceCount, firstIndex, baseVertex,
    /// firstInstance)`.
    pub fn draw_indexed(
        pass: u32,
        index_count: u32,
        instance_count: Option<u32>,
        first_index: Option<u32>,
        base_vertex: Option<i32>,
        first_instance: Option<u32>,
    ) -> Result<(), String> {
        draw(pass, "drawIndexed", || {
            Ok(Ok(DrawCommand::DrawIndexed(
                span(first_index, index_count),
                base_vertex.unwrap_or(0),
                span(first_instance, instance_count.unwrap_or(1)),
            )))
        })
    }

    /// `pass.drawIndirect(indirectBuffer, indirectOffset)`.
    pub fn draw_indirect(
        pass: u32,
        indirect_buffer: u32,
        indirect_offset: f64,
    ) -> Result<(), String> {
        draw(pass, "drawIndirect", || {
            draw_indirect(false, indirect_buffer, indirect_offset)
        })
    }

    /// `pass.drawIndexedIndirect(indirectBuffer, indirectOffset)`.
    pub fn draw_indexed_indirect(
        pass: u32,
        indirect_buffer: u32,
        indirect_offset: f64,
    ) -> Result<(), String> {
        draw(pass, "drawIndexedIndirect", || {
            draw_indirect(true, indirect_buffer, indirect_offset)
        })
    }

    /// `pass.setImmediates(rangeOffset, data)` with the bytes TSX cut.
    pub fn set_immediates(pass: u32, range_offset: u32, data: Bytes) -> Result<(), String> {
        draw(pass, "setImmediates", || {
            Ok(immediates(range_offset, data).map(|(o, d)| DrawCommand::SetImmediates(o, d)))
        })
    }

    /// `pass.setViewport(x, y, width, height, minDepth, maxDepth)`.
    pub fn set_viewport(
        pass: u32,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    ) -> Result<(), String> {
        rec(pass, "setViewport", || {
            if !(0.0..=1.0).contains(&min_depth)
                || !(0.0..=1.0).contains(&max_depth)
                || min_depth > max_depth
                || width < 0.0
                || height < 0.0
            {
                return Ok(Err(format!(
                    "width {width} and height {height} must be >= 0, and 0 <= minDepth {min_depth} <= maxDepth {max_depth} <= 1"
                )));
            }
            Ok(Ok(RenderCommand::SetViewport([
                x, y, width, height, min_depth, max_depth,
            ])))
        })
    }

    /// `pass.setScissorRect(x, y, width, height)`.
    pub fn set_scissor_rect(
        pass: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        rec(pass, "setScissorRect", || {
            Ok(Ok(RenderCommand::SetScissorRect([x, y, width, height])))
        })
    }

    /// `pass.setBlendConstant(color)`.
    pub fn set_blend_constant(pass: u32, color: JsonValue) -> Result<(), String> {
        rec(pass, "setBlendConstant", || {
            let color = parse::<Color>("GPUColor", color)?.to_wgpu()?;
            Ok(Ok(RenderCommand::SetBlendConstant(color)))
        })
    }

    /// `pass.setStencilReference(reference)`.
    pub fn set_stencil_reference(pass: u32, reference: u32) -> Result<(), String> {
        rec(pass, "setStencilReference", || {
            Ok(Ok(RenderCommand::SetStencilReference(reference)))
        })
    }

    /// `pass.beginOcclusionQuery(queryIndex)`.
    pub fn begin_occlusion_query(pass: u32, query_index: u32) -> Result<(), String> {
        let set = guard(|| {
            registry::with_mut::<RenderPassEntry, _>(pass, |p| p.target.occlusion.clone())
        })?;
        rec(pass, "beginOcclusionQuery", || {
            Ok(match set {
                None => Err("the pass has no occlusionQuerySet".into()),
                Some(set) => match command_encoder::query_set_problem(
                    &set,
                    "occlusion",
                    "beginOcclusionQuery",
                ) {
                    Some(problem) => Err(problem),
                    None if query_index >= set.count => Err(format!(
                        "queryIndex {query_index} is not below the set's {} queries",
                        set.count
                    )),
                    None => Ok(RenderCommand::BeginOcclusionQuery(query_index)),
                },
            })
        })
    }

    /// `pass.endOcclusionQuery()`.
    pub fn end_occlusion_query(pass: u32) -> Result<(), String> {
        rec(pass, "endOcclusionQuery", || {
            Ok(Ok(RenderCommand::EndOcclusionQuery))
        })
    }

    /// `pass.executeBundles(bundles)`.
    pub fn execute_bundles(pass: u32, bundles: Vec<u32>) -> Result<(), String> {
        rec(pass, "executeBundles", || {
            let mut out = Vec::with_capacity(bundles.len());
            for id in bundles {
                match registry::get::<RenderBundleEntry>(id)?.bundle {
                    Some(b) => out.push(b),
                    None => return Ok(Err(format!("render bundle {id} is invalid"))),
                }
            }
            Ok(Ok(RenderCommand::ExecuteBundles(out)))
        })
    }

    /// `pass.pushDebugGroup(groupLabel)`.
    pub fn push_debug_group(pass: u32, group_label: String) -> Result<(), String> {
        rec(pass, "pushDebugGroup", || {
            Ok(Ok(RenderCommand::PushDebugGroup(group_label)))
        })
    }

    /// `pass.popDebugGroup()`.
    pub fn pop_debug_group(pass: u32) -> Result<(), String> {
        rec(pass, "popDebugGroup", || {
            Ok(Ok(RenderCommand::PopDebugGroup))
        })
    }

    /// `pass.insertDebugMarker(markerLabel)`.
    pub fn insert_debug_marker(pass: u32, marker_label: String) -> Result<(), String> {
        rec(pass, "insertDebugMarker", || {
            Ok(Ok(RenderCommand::InsertDebugMarker(marker_label)))
        })
    }

    /// `pass.end()`: replays the pass into its encoder (or, when it is
    /// invalid, makes the encoder invalid) and unlocks the encoder. A
    /// second `end()` is a validation error.
    pub fn end(pass: u32) -> Result<(), String> {
        guard(|| {
            let taken = registry::with_mut::<RenderPassEntry, _>(pass, |p| {
                if p.ended {
                    return Err(p.device);
                }
                p.ended = true;
                Ok((
                    p.encoder,
                    p.error.take(),
                    std::mem::take(&mut p.commands),
                    std::mem::replace(&mut p.target, OwnedRenderPass::empty()),
                ))
            })?;
            let (encoder, error, commands, target) = match taken {
                Ok(t) => t,
                Err(device) => {
                    DeviceEntry::get(device)?.report(
                        GpuErrorKind::Validation,
                        "end: the render pass has already ended",
                    );
                    return Ok(());
                }
            };
            command_encoder::unlock(encoder, error, |e| {
                target.run(e, |p| replay(p, &commands));
            })
        })
    }
}
