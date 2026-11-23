//! `GPUComputePassEncoder`: records its commands (with owned wgpu handles)
//! until `end()`, which replays them into a wgpu compute pass on the
//! encoder (`command_encoder.rs` explains why).

use nsh_core::prelude::*;

use crate::{
    api::{
        bind_group::BindGroupEntry,
        buffer::BufferEntry,
        command_encoder::{self, OwnedComputePass},
        compute_pipeline::ComputePipelineEntry,
        device::DeviceEntry,
        render_pass_encoder::{Made, Recording, immediates, record},
    },
    bytes::Bytes,
    error::{GpuErrorKind, NativeError, guard},
    registry::{self, Id},
};

/// What a compute pass records.
pub(crate) enum ComputeCommand {
    SetPipeline(wgpu::ComputePipeline),
    SetBindGroup(u32, Option<wgpu::BindGroup>, Vec<u32>),
    Dispatch(u32, u32, u32),
    DispatchIndirect(wgpu::Buffer, u64),
    SetImmediates(u32, Vec<u8>),
    PushDebugGroup(String),
    PopDebugGroup,
    InsertDebugMarker(String),
}

fn replay(pass: &mut wgpu::ComputePass<'_>, commands: &[ComputeCommand]) {
    for command in commands {
        match command {
            ComputeCommand::SetPipeline(p) => pass.set_pipeline(p),
            ComputeCommand::SetBindGroup(i, g, offsets) => {
                pass.set_bind_group(*i, g.as_ref(), offsets)
            }
            ComputeCommand::Dispatch(x, y, z) => pass.dispatch_workgroups(*x, *y, *z),
            ComputeCommand::DispatchIndirect(b, o) => pass.dispatch_workgroups_indirect(b, *o),
            ComputeCommand::SetImmediates(o, d) => pass.set_immediates(*o, d),
            ComputeCommand::PushDebugGroup(l) => pass.push_debug_group(l),
            ComputeCommand::PopDebugGroup => pass.pop_debug_group(),
            ComputeCommand::InsertDebugMarker(l) => pass.insert_debug_marker(l),
        }
    }
}

/// An open (or ended) compute pass in the registry.
pub struct ComputePassEntry {
    /// Its device.
    pub device: Id,
    /// Its encoder.
    pub encoder: Id,
    target: Option<OwnedComputePass>,
    commands: Vec<ComputeCommand>,
    error: Option<String>,
    ended: bool,
}

impl ComputePassEntry {
    pub(crate) fn new(
        device: Id,
        encoder: Id,
        target: OwnedComputePass,
        error: Option<String>,
    ) -> Self {
        Self {
            device,
            encoder,
            target: Some(target),
            commands: Vec::new(),
            error,
            ended: false,
        }
    }
}

impl Recording for ComputePassEntry {
    type Command = ComputeCommand;
    const KIND: &'static str = "compute pass";
    fn device(&self) -> Id {
        self.device
    }
    fn ended(&self) -> bool {
        self.ended
    }
    fn commands(&mut self) -> &mut Vec<ComputeCommand> {
        &mut self.commands
    }
    fn error(&mut self) -> &mut Option<String> {
        &mut self.error
    }
}

fn rec(
    pass: u32,
    what: &str,
    make: impl FnOnce() -> Result<Made<ComputeCommand>, NativeError>,
) -> Result<(), String> {
    guard(|| record::<ComputePassEntry>(pass, what, make))
}

/// The native half of `GPUComputePassEncoder` (begun by
/// `WgpuCommandEncoder.beginComputePass`).
#[module]
pub struct WgpuComputePassEncoder;

#[module]
impl WgpuComputePassEncoder {
    /// `pass.setPipeline(pipeline)`.
    pub fn set_pipeline(pass: u32, pipeline: u32) -> Result<(), String> {
        rec(pass, "setPipeline", || {
            let p = registry::get::<ComputePipelineEntry>(pipeline)?.pipeline;
            Ok(Ok(ComputeCommand::SetPipeline(p)))
        })
    }

    /// `pass.setBindGroup(index, bindGroup, dynamicOffsets)`.
    pub fn set_bind_group(
        pass: u32,
        index: u32,
        bind_group: Option<u32>,
        dynamic_offsets: Vec<u32>,
    ) -> Result<(), String> {
        rec(pass, "setBindGroup", || {
            let group = bind_group
                .map(registry::get::<BindGroupEntry>)
                .transpose()?;
            if group.as_ref().is_some_and(|g| !g.valid) {
                return Ok(Err("the bind group is invalid".into()));
            }
            Ok(Ok(ComputeCommand::SetBindGroup(
                index,
                group.map(|g| g.bind_group),
                dynamic_offsets,
            )))
        })
    }

    /// `pass.dispatchWorkgroups(x, y, z)`.
    pub fn dispatch_workgroups(
        pass: u32,
        workgroup_count_x: u32,
        workgroup_count_y: Option<u32>,
        workgroup_count_z: Option<u32>,
    ) -> Result<(), String> {
        rec(pass, "dispatchWorkgroups", || {
            Ok(Ok(ComputeCommand::Dispatch(
                workgroup_count_x,
                workgroup_count_y.unwrap_or(1),
                workgroup_count_z.unwrap_or(1),
            )))
        })
    }

    /// `pass.dispatchWorkgroupsIndirect(indirectBuffer, indirectOffset)`.
    pub fn dispatch_workgroups_indirect(
        pass: u32,
        indirect_buffer: u32,
        indirect_offset: f64,
    ) -> Result<(), String> {
        rec(pass, "dispatchWorkgroupsIndirect", || {
            let offset = command_encoder::enforce_u64("indirectOffset", indirect_offset)?;
            let buffer = registry::get::<BufferEntry>(indirect_buffer)?.buffer;
            if !offset.is_multiple_of(4) || offset.checked_add(12).is_none_or(|e| e > buffer.size())
            {
                return Ok(Err(format!(
                    "indirectOffset {offset} must be a multiple of 4 with 12 bytes of arguments inside the buffer's {} bytes",
                    buffer.size()
                )));
            }
            Ok(Ok(ComputeCommand::DispatchIndirect(buffer, offset)))
        })
    }

    /// `pass.setImmediates(rangeOffset, data)` with the bytes TSX cut.
    pub fn set_immediates(pass: u32, range_offset: u32, data: Bytes) -> Result<(), String> {
        rec(pass, "setImmediates", || {
            Ok(immediates(range_offset, data).map(|(o, d)| ComputeCommand::SetImmediates(o, d)))
        })
    }

    /// `pass.pushDebugGroup(groupLabel)`.
    pub fn push_debug_group(pass: u32, group_label: String) -> Result<(), String> {
        rec(pass, "pushDebugGroup", || {
            Ok(Ok(ComputeCommand::PushDebugGroup(group_label)))
        })
    }

    /// `pass.popDebugGroup()`.
    pub fn pop_debug_group(pass: u32) -> Result<(), String> {
        rec(pass, "popDebugGroup", || {
            Ok(Ok(ComputeCommand::PopDebugGroup))
        })
    }

    /// `pass.insertDebugMarker(markerLabel)`.
    pub fn insert_debug_marker(pass: u32, marker_label: String) -> Result<(), String> {
        rec(pass, "insertDebugMarker", || {
            Ok(Ok(ComputeCommand::InsertDebugMarker(marker_label)))
        })
    }

    /// `pass.end()`: replays the pass into its encoder (or makes the
    /// encoder invalid) and unlocks it. A second `end()` is a validation
    /// error.
    pub fn end(pass: u32) -> Result<(), String> {
        guard(|| {
            let taken = registry::with_mut::<ComputePassEntry, _>(pass, |p| {
                if p.ended {
                    return Err(p.device);
                }
                p.ended = true;
                Ok((
                    p.encoder,
                    p.error.take(),
                    std::mem::take(&mut p.commands),
                    p.target.take(),
                ))
            })?;
            let (encoder, error, commands, target) = match taken {
                Ok(t) => t,
                Err(device) => {
                    DeviceEntry::get(device)?.report(
                        GpuErrorKind::Validation,
                        "end: the compute pass has already ended",
                    );
                    return Ok(());
                }
            };
            command_encoder::unlock(encoder, error, |e| {
                if let Some(target) = &target {
                    target.run(e, |p| replay(p, &commands));
                }
            })
        })
    }
}

/// The compute pass through this module's functions, with the pipeline and
/// bind groups made by wgpu directly (area (a)'s `createComputePipeline`
/// and `createBindGroup` are stubs in this tree; `tests/commands/compute.rs`
/// has the same through the public API, ignored until they land).
#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::api::{buffer::WgpuBuffer, command_encoder::WgpuCommandEncoder, queue::WgpuQueue};

    /// A device in the registry, and its wgpu device.
    fn device() -> (Id, wgpu::Device) {
        let adapter =
            pollster::block_on(crate::instance::instance().request_adapter(&Default::default()))
                .expect("an adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).expect("a device");
        (DeviceEntry::register(device.clone(), queue, None), device)
    }

    /// A buffer made by `WgpuBuffer.create`, holding `bytes`; its id and
    /// wgpu buffer.
    fn buffer(device: Id, usage: u32, bytes: &[u8]) -> (Id, wgpu::Buffer) {
        let id = WgpuBuffer::create(device, json!({ "size": bytes.len(), "usage": usage | 0x8 }))
            .unwrap();
        WgpuQueue::write_buffer(device, id, 0.0, Bytes(bytes.to_vec())).unwrap();
        (id, registry::get::<BufferEntry>(id).unwrap().buffer)
    }

    /// `buffer`'s bytes once the queue is idle.
    fn read(device: &wgpu::Device, queue_device: Id, buffer: Id, size: u64) -> Vec<u8> {
        let out =
            WgpuBuffer::create(queue_device, json!({ "size": size, "usage": 0x1 | 0x8 })).unwrap();
        let encoder = WgpuCommandEncoder::create(queue_device, json!(null)).unwrap();
        WgpuCommandEncoder::copy_buffer_to_buffer(
            encoder,
            buffer,
            0.0,
            out,
            0.0,
            Some(size as f64),
        )
        .unwrap();
        let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
        WgpuQueue::submit(queue_device, vec![commands]).unwrap();
        let out = registry::get::<BufferEntry>(out).unwrap().buffer;
        out.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let bytes = out.slice(..).get_mapped_range().to_vec();
        out.unmap();
        bytes
    }

    fn floats(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    const MATMUL: &str = "
struct Matrix { size : vec2<f32>, numbers: array<f32> }
@group(0) @binding(0) var<storage, read> firstMatrix : Matrix;
@group(0) @binding(1) var<storage, read> secondMatrix : Matrix;
@group(0) @binding(2) var<storage, read_write> resultMatrix : Matrix;
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id : vec3<u32>) {
  if (global_id.x >= u32(firstMatrix.size.x) || global_id.y >= u32(secondMatrix.size.y)) { return; }
  resultMatrix.size = vec2(firstMatrix.size.x, secondMatrix.size.y);
  let resultCell = vec2(global_id.x, global_id.y);
  var result = 0.0;
  for (var i = 0u; i < u32(firstMatrix.size.y); i = i + 1u) {
    let a = i + resultCell.x * u32(firstMatrix.size.y);
    let b = resultCell.y + i * u32(secondMatrix.size.y);
    result = result + firstMatrix.numbers[a] * secondMatrix.numbers[b];
  }
  let index = resultCell.y + resultCell.x * u32(secondMatrix.size.y);
  resultMatrix.numbers[index] = result;
}";

    #[test]
    fn matrix_multiplication_direct_and_indirect() {
        // ComputeShader.spec.ts "matrix multiplication".
        let (device, wgpu_device) = device();
        let m = |seed: u32| -> Vec<f32> {
            let mut v = vec![16.0, 16.0];
            let mut x = seed;
            for _ in 0..256 {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                v.push((x >> 16) as f32 / 65536.0);
            }
            v
        };
        let (m1, m2) = (m(1), m(2));
        let bytes = |v: &[f32]| v.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>();
        let (_, a) = buffer(device, 0x80, &bytes(&m1));
        let (_, b) = buffer(device, 0x80, &bytes(&m2));
        let size = 4 * 258;
        let (result_id, result) = buffer(device, 0x80 | 0x4, &vec![0; size]);
        let (indirect_id, _) = buffer(device, 0x100, &[2, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0]);
        let module = wgpu_device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(MATMUL.into()),
        });
        let pipeline = wgpu_device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: a.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: b.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: result.as_entire_binding(),
                },
            ],
        });
        let pipeline = registry::insert(ComputePipelineEntry { pipeline, device });
        let group = registry::insert(BindGroupEntry {
            bind_group: group,
            device,
            valid: true,
        });
        for indirect in [false, true] {
            let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
            WgpuCommandEncoder::clear_buffer(encoder, result_id, None, None).unwrap();
            let pass =
                WgpuCommandEncoder::begin_compute_pass(encoder, json!({ "label": "matmul" }))
                    .unwrap();
            WgpuComputePassEncoder::set_pipeline(pass, pipeline).unwrap();
            WgpuComputePassEncoder::set_bind_group(pass, 0, Some(group), vec![]).unwrap();
            WgpuComputePassEncoder::push_debug_group(pass, "dispatch".into()).unwrap();
            if indirect {
                WgpuComputePassEncoder::dispatch_workgroups_indirect(pass, indirect_id, 0.0)
                    .unwrap();
            } else {
                WgpuComputePassEncoder::dispatch_workgroups(pass, 2, Some(2), None).unwrap();
            }
            WgpuComputePassEncoder::pop_debug_group(pass).unwrap();
            WgpuComputePassEncoder::end(pass).unwrap();
            let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
            WgpuQueue::submit(device, vec![commands]).unwrap();
            let out = floats(&read(&wgpu_device, device, result_id, size as u64));
            assert_eq!(out.len(), 258);
            assert_eq!(&out[..2], &[16.0, 16.0]);
            for i in 0..16 {
                for j in 0..16 {
                    let expected: f32 = (0..16)
                        .map(|k| m1[2 + i * 16 + k] * m2[2 + k * 16 + j])
                        .sum();
                    assert!(
                        (out[2 + i * 16 + j] - expected).abs() < 1e-4,
                        "indirect={indirect} ({i}, {j})"
                    );
                }
            }
        }
    }

    #[test]
    fn dynamic_offsets_reach_the_shader() {
        // setBindGroup(index, group, [offset]): the shader copies the
        // uniform at the dynamic offset into the output.
        let (device, wgpu_device) = device();
        let mut uniforms = vec![0u8; 512];
        uniforms[256..260].copy_from_slice(&42u32.to_le_bytes());
        uniforms[0..4].copy_from_slice(&7u32.to_le_bytes());
        let (_, uniform) = buffer(device, 0x40, &uniforms);
        let (out_id, out) = buffer(device, 0x80 | 0x4, &[0; 4]);
        let module = wgpu_device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(
                "@group(0) @binding(0) var<uniform> u: u32;
                 @group(0) @binding(1) var<storage, read_write> o: u32;
                 @compute @workgroup_size(1) fn main() { o = u; }"
                    .into(),
            ),
        });
        let layout = wgpu_device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = wgpu_device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = wgpu_device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniform,
                        offset: 0,
                        size: std::num::NonZeroU64::new(4),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: out.as_entire_binding(),
                },
            ],
        });
        let pipeline = registry::insert(ComputePipelineEntry { pipeline, device });
        let group = registry::insert(BindGroupEntry {
            bind_group: group,
            device,
            valid: true,
        });
        for (offset, expected) in [(256, 42u32), (0, 7)] {
            let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
            let pass = WgpuCommandEncoder::begin_compute_pass(encoder, json!(null)).unwrap();
            WgpuComputePassEncoder::set_pipeline(pass, pipeline).unwrap();
            WgpuComputePassEncoder::set_bind_group(pass, 0, Some(group), vec![offset]).unwrap();
            WgpuComputePassEncoder::dispatch_workgroups(pass, 1, None, None).unwrap();
            WgpuComputePassEncoder::end(pass).unwrap();
            let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
            WgpuQueue::submit(device, vec![commands]).unwrap();
            assert_eq!(
                read(&wgpu_device, device, out_id, 4),
                expected.to_le_bytes()
            );
        }
    }
}
