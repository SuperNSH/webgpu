//! `ComputeShader.spec.ts` (matrix multiplication) through the public
//! API, with area (a)'s layouts, pipelines and bind groups.

use nsh_webgpu::api::{
    bind_group::WgpuBindGroup, bind_group_layout::WgpuBindGroupLayout,
    command_encoder::WgpuCommandEncoder, compute_pass_encoder::WgpuComputePassEncoder as Pass,
    compute_pipeline::WgpuComputePipeline, pipeline_layout::WgpuPipelineLayout, queue::WgpuQueue,
    shader_module::WgpuShaderModule,
};
use serde_json::json;

use crate::common::*;

pub(crate) const MATMUL_WGSL: &str = "
struct Matrix {
  size : vec2<f32>,
  numbers: array<f32>,
}
@group(0) @binding(0) var<storage, read> firstMatrix : Matrix;
@group(0) @binding(1) var<storage, read> secondMatrix : Matrix;
@group(0) @binding(2) var<storage, read_write> resultMatrix : Matrix;
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id : vec3<u32>) {
  if (global_id.x >= u32(firstMatrix.size.x) || global_id.y >= u32(secondMatrix.size.y)) {
    return;
  }
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

/// A 16 x 16 matrix in the shader's layout (size, then numbers).
fn matrix(seed: u32) -> Vec<f32> {
    let mut m = vec![16.0, 16.0];
    let mut x = seed;
    for _ in 0..256 {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        m.push((x >> 16) as f32 / 65536.0);
    }
    m
}

#[test]
fn matrix_multiplication() {
    let device = device();
    let (m1, m2) = (matrix(1), matrix(2));
    let a = buffer_with(device, usage::STORAGE, &f32s(&m1));
    let b = buffer_with(device, usage::STORAGE, &f32s(&m2));
    let size = 4 * (2 + 256);
    let result = buffer_with(device, usage::STORAGE | usage::COPY_SRC, &vec![0; size]);
    let module = WgpuShaderModule::create(device, json!({ "code": MATMUL_WGSL })).unwrap();
    let storage = |binding: u32, kind: &str| json!({ "binding": binding, "visibility": 4, "buffer": { "type": kind } });
    let layout = WgpuBindGroupLayout::create(
        device,
        json!({ "entries": [storage(0, "read-only-storage"), storage(1, "read-only-storage"), storage(2, "storage")] }),
    )
    .unwrap();
    let pipeline_layout =
        WgpuPipelineLayout::create(device, json!({ "bindGroupLayouts": [layout] })).unwrap();
    let pipeline = WgpuComputePipeline::create(
        device,
        json!({ "layout": pipeline_layout, "compute": { "module": module, "entryPoint": "main" } }),
    )
    .unwrap();
    let group = WgpuBindGroup::create(
        device,
        json!({ "layout": layout, "entries": [
            { "binding": 0, "resource": { "buffer": a } },
            { "binding": 1, "resource": { "buffer": b } },
            { "binding": 2, "resource": { "buffer": result } },
        ] }),
    )
    .unwrap();
    let encoder = WgpuCommandEncoder::create(device, json!(null)).unwrap();
    let pass = WgpuCommandEncoder::begin_compute_pass(encoder, json!(null)).unwrap();
    Pass::set_pipeline(pass, pipeline).unwrap();
    Pass::set_bind_group(pass, 0, Some(group), vec![]).unwrap();
    Pass::dispatch_workgroups(pass, 2, Some(2), None).unwrap();
    Pass::end(pass).unwrap();
    let commands = WgpuCommandEncoder::finish(encoder, json!(null)).unwrap();
    WgpuQueue::submit(device, vec![commands]).unwrap();
    let out: Vec<f32> = read_back(device, result, size as u64)
        .chunks(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    assert_eq!(out.len(), 258);
    for i in 0..16 {
        for j in 0..16 {
            let expected: f32 = (0..16)
                .map(|k| m1[2 + i * 16 + k] * m2[2 + k * 16 + j])
                .sum();
            assert!((out[2 + i * 16 + j] - expected).abs() < 1e-4);
        }
    }
}
