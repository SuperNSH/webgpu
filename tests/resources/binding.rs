//! Samplers, bind group layouts, bind groups, pipeline layouts and compute
//! pipelines: the reference's SamplerCompare.spec, ComputeShader.spec and
//! NullSlots.spec (bind group layout), Device.spec (raw buffers in bind
//! groups), ErrorScope.spec (an invalid sampler), and the spec's entry
//! types. The compute passes are recorded with wgpu directly on the
//! module's objects: `beginComputePass` is another area's.

use nsh_webgpu::{
    api::{
        bind_group::{BindGroupEntry, WgpuBindGroup},
        bind_group_layout::WgpuBindGroupLayout,
        buffer::{BufferEntry, WgpuBuffer},
        compute_pipeline::{ComputePipelineEntry, WgpuComputePipeline},
        pipeline_layout::WgpuPipelineLayout,
        render_pipeline::WgpuRenderPipeline,
        sampler::{SamplerEntry, WgpuSampler},
        shader_module::WgpuShaderModule,
        texture::{TextureEntry, WgpuTexture},
        texture_view::WgpuTextureView,
    },
    bytes::Bytes,
};
use serde_json::json;

use crate::common::{assert_throws, device, submit, validation_error, wait, wgpu};

const MAP_READ: u32 = 0x1;
const COPY_SRC: u32 = 0x4;
const COPY_DST: u32 = 0x8;
const UNIFORM: u32 = 0x40;
const STORAGE: u32 = 0x80;
const TEXTURE_BINDING: u32 = 0x04;
const TEXTURE_COPY_DST: u32 = 0x02;
const STORAGE_BINDING: u32 = 0x08;
const VERTEX_STAGE: u32 = 0x1;
const FRAGMENT_STAGE: u32 = 0x2;
const COMPUTE_STAGE: u32 = 0x4;

/// Runs compute pipeline `pipeline` with `groups` bound, `x` × `y`
/// workgroups, then copies `out` (`size` bytes) to a MAP_READ buffer and
/// reads it back through the module's mapping.
fn dispatch(
    device: u32,
    pipeline: u32,
    groups: &[(u32, u32)],
    x: u32,
    y: u32,
    out: u32,
    size: u64,
) -> Vec<u8> {
    let pipeline = ComputePipelineEntry::lookup(pipeline).unwrap();
    let groups: Vec<(u32, BindGroupEntry)> = groups
        .iter()
        .map(|(index, id)| (*index, BindGroupEntry::lookup(*id).unwrap()))
        .collect();
    let readback = WgpuBuffer::create(
        device,
        json!({ "size": size, "usage": MAP_READ | COPY_DST }),
    )
    .unwrap();
    let (src, dst) = (
        BufferEntry::lookup(out).unwrap(),
        BufferEntry::lookup(readback).unwrap(),
    );
    submit(device, |e| {
        {
            let mut pass = e.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&pipeline.pipeline);
            for (index, group) in &groups {
                pass.set_bind_group(*index, &group.bind_group, &[]);
            }
            pass.dispatch_workgroups(x, y, 1);
        }
        e.copy_buffer_to_buffer(&src.buffer, 0, &dst.buffer, 0, size);
    });
    wait(WgpuBuffer::map_async(readback, MAP_READ, None, None).unwrap()).unwrap();
    WgpuBuffer::get_mapped_range(readback, None, None).unwrap()
}

fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

/// SamplerCompare.spec (#624): a depth texture through a compare sampler.
#[test]
fn a_compare_sampler_samples_a_depth_texture() {
    let device = device();
    let (depth, error) = validation_error(device, || {
        WgpuTexture::create(
            device,
            json!({ "size": [2, 1], "format": "depth16unorm", "usage": TEXTURE_BINDING | TEXTURE_COPY_DST }),
        )
        .unwrap()
    });
    assert_eq!(error, None, "the depth texture");
    // writeTexture is another area's; the same upload through wgpu.
    let (_, queue) = wgpu(device);
    let texels: Vec<u8> = [16384u16, 49152]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    queue.write_texture(
        TextureEntry::lookup(depth).unwrap().texture.as_image_copy(),
        &texels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: 2,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let (values, error) = validation_error(device, || {
        let sampler = WgpuSampler::create(device, json!({ "compare": "less" })).unwrap();
        assert!(SamplerEntry::lookup(sampler).unwrap().comparison);
        let out =
            WgpuBuffer::create(device, json!({ "size": 8, "usage": STORAGE | COPY_SRC })).unwrap();
        let module = WgpuShaderModule::create(
            device,
            json!({ "code": "
@group(0) @binding(0) var depthTex: texture_depth_2d;
@group(0) @binding(1) var cmp: sampler_comparison;
@group(0) @binding(2) var<storage, read_write> result: array<f32, 2>;

@compute @workgroup_size(1)
fn main() {
  result[0] = textureSampleCompareLevel(depthTex, cmp, vec2f(0.25, 0.5), 0.5);
  result[1] = textureSampleCompareLevel(depthTex, cmp, vec2f(0.75, 0.5), 0.5);
}" }),
        )
        .unwrap();
        let pipeline = WgpuComputePipeline::create(
            device,
            json!({ "layout": "auto", "compute": { "module": module, "entryPoint": "main" } }),
        )
        .unwrap();
        let layout = WgpuComputePipeline::get_bind_group_layout(pipeline, 0).unwrap();
        let view = WgpuTextureView::create(depth, json!(null)).unwrap();
        let group = WgpuBindGroup::create(
            device,
            json!({ "layout": layout, "entries": [
                { "binding": 0, "resource": view },
                { "binding": 1, "resource": sampler },
                { "binding": 2, "resource": { "buffer": out } },
            ] }),
        )
        .unwrap();
        floats(&dispatch(device, pipeline, &[(0, group)], 1, 1, out, 8))
    });
    assert_eq!(error, None);
    assert_eq!(values, vec![0.0, 1.0]);
}

/// ComputeShader.spec "matrix multiplication": 16 × 16, against the CPU.
#[test]
fn a_compute_pipeline_multiplies_matrices() {
    let device = device();
    let n = 16usize;
    let mut seed = 0x2545_f491_u32;
    let mut random = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed % 10_000) as f32 / 10_000.0
    };
    let mut m1 = vec![n as f32, n as f32];
    let mut m2 = m1.clone();
    m1.extend((0..n * n).map(|_| random()));
    m2.extend((0..n * n).map(|_| random()));
    let matrix = |m: &[f32]| {
        let bytes: Vec<u8> = m.iter().flat_map(|f| f.to_le_bytes()).collect();
        let id = WgpuBuffer::create(
            device,
            json!({ "size": bytes.len(), "usage": STORAGE, "mappedAtCreation": true }),
        )
        .unwrap();
        WgpuBuffer::write_mapped(id, 0.0, Bytes(bytes)).unwrap();
        WgpuBuffer::unmap(id).unwrap();
        id
    };
    let (first, second) = (matrix(&m1), matrix(&m2));
    let size = 4 * (2 + n * n) as u64;
    let result =
        WgpuBuffer::create(device, json!({ "size": size, "usage": STORAGE | COPY_SRC })).unwrap();
    let module = WgpuShaderModule::create(
        device,
        json!({ "code": "
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
      }" }),
    )
    .unwrap();
    let pipeline = WgpuComputePipeline::create(
        device,
        json!({ "layout": "auto", "compute": { "module": module, "entryPoint": "main" } }),
    )
    .unwrap();
    let group = WgpuBindGroup::create(
        device,
        json!({
            "layout": WgpuComputePipeline::get_bind_group_layout(pipeline, 0).unwrap(),
            "entries": [
                { "binding": 0, "resource": { "buffer": first } },
                { "binding": 1, "resource": { "buffer": second } },
                { "binding": 2, "resource": { "buffer": result } },
            ],
        }),
    )
    .unwrap();
    let got = floats(&dispatch(
        device,
        pipeline,
        &[(0, group)],
        2,
        2,
        result,
        size,
    ));
    assert_eq!(got.len(), n * n + 2);
    assert_eq!(&got[..2], &[n as f32, n as f32]);
    for i in 0..n {
        for j in 0..n {
            let expected: f32 = (0..n).map(|k| m1[2 + i * n + k] * m2[2 + k * n + j]).sum();
            let actual = got[2 + i * n + j];
            assert!(
                (actual - expected).abs() < 1e-4,
                "[{i}][{j}] {actual} != {expected}"
            );
        }
    }
}

/// NullSlots.spec "keeps the index of a bind group layout after a null
/// one", with an explicit layout and a raw GPUBuffer (Device.spec).
#[test]
fn a_null_bind_group_layout_keeps_the_later_indices() {
    let device = device();
    let module = WgpuShaderModule::create(
        device,
        json!({ "code": "
          @group(1) @binding(0) var<storage, read_write> out: u32;
          @compute @workgroup_size(1) fn main() { out = 42u; }" }),
    )
    .unwrap();
    let layout = WgpuBindGroupLayout::create(
        device,
        json!({ "entries": [{ "binding": 0, "visibility": COMPUTE_STAGE, "buffer": { "type": "storage" } }] }),
    )
    .unwrap();
    let storage =
        WgpuBuffer::create(device, json!({ "size": 4, "usage": STORAGE | COPY_SRC })).unwrap();
    let (value, error) = validation_error(device, || {
        let pipeline_layout =
            WgpuPipelineLayout::create(device, json!({ "bindGroupLayouts": [null, layout] }))
                .unwrap();
        let pipeline = WgpuComputePipeline::create(
            device,
            json!({ "layout": pipeline_layout, "compute": { "module": module } }),
        )
        .unwrap();
        // The whole buffer, given as the GPUBuffer itself.
        let group = WgpuBindGroup::create(
            device,
            json!({ "layout": layout, "entries": [{ "binding": 0, "resource": storage }] }),
        )
        .unwrap();
        dispatch(device, pipeline, &[(1, group)], 1, 1, storage, 4)
    });
    assert_eq!(error, None);
    assert_eq!(value, 42u32.to_le_bytes());
}

#[test]
fn bind_group_layouts_take_every_entry_type() {
    let device = device();
    let all = VERTEX_STAGE | FRAGMENT_STAGE | COMPUTE_STAGE;
    let (layout, error) = validation_error(device, || {
        WgpuBindGroupLayout::create(
            device,
            json!({ "label": "everything", "entries": [
                { "binding": 0, "visibility": all, "buffer": {} },
                { "binding": 1, "visibility": COMPUTE_STAGE, "buffer": { "type": "storage", "hasDynamicOffset": true, "minBindingSize": 16 } },
                { "binding": 2, "visibility": all, "buffer": { "type": "read-only-storage" } },
                { "binding": 3, "visibility": FRAGMENT_STAGE, "sampler": {} },
                { "binding": 4, "visibility": FRAGMENT_STAGE, "sampler": { "type": "non-filtering" } },
                { "binding": 5, "visibility": FRAGMENT_STAGE, "sampler": { "type": "comparison" } },
                { "binding": 6, "visibility": FRAGMENT_STAGE, "texture": {} },
                { "binding": 7, "visibility": FRAGMENT_STAGE, "texture": { "sampleType": "unfilterable-float", "viewDimension": "2d-array" } },
                { "binding": 8, "visibility": FRAGMENT_STAGE, "texture": { "sampleType": "depth", "viewDimension": "cube" } },
                { "binding": 9, "visibility": FRAGMENT_STAGE, "texture": { "sampleType": "sint", "viewDimension": "3d" } },
                { "binding": 10, "visibility": FRAGMENT_STAGE, "texture": { "sampleType": "uint", "multisampled": true } },
                { "binding": 11, "visibility": COMPUTE_STAGE, "storageTexture": { "format": "rgba8unorm" } },
                { "binding": 12, "visibility": COMPUTE_STAGE, "storageTexture": { "access": "read-only", "format": "r32float", "viewDimension": "2d-array" } },
                { "binding": 13, "visibility": COMPUTE_STAGE, "storageTexture": { "access": "read-write", "format": "r32uint" } },
            ] }),
        )
        .unwrap()
    });
    assert_eq!(error, None);
    // Bind all of it: buffers (whole, ranges), samplers, views and a
    // texture standing for its default view.
    let buffer =
        |usage: u32| WgpuBuffer::create(device, json!({ "size": 256, "usage": usage })).unwrap();
    let texture = |format: &str, size: serde_json::Value, usage: u32, extra: serde_json::Value| {
        let mut d = json!({ "size": size, "format": format, "usage": usage });
        d.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        WgpuTexture::create(device, d).unwrap()
    };
    let float2d = texture("rgba8unorm", json!([4, 4]), TEXTURE_BINDING, json!({}));
    let layers = texture("rgba32float", json!([4, 4, 2]), TEXTURE_BINDING, json!({}));
    let cube = texture("depth32float", json!([4, 4, 6]), TEXTURE_BINDING, json!({}));
    let volume = texture(
        "rgba8sint",
        json!([4, 4, 4]),
        TEXTURE_BINDING,
        json!({ "dimension": "3d" }),
    );
    let msaa = texture(
        "rgba8uint",
        json!([4, 4]),
        TEXTURE_BINDING | 0x10,
        json!({ "sampleCount": 4 }),
    );
    let write = texture("rgba8unorm", json!([4, 4]), STORAGE_BINDING, json!({}));
    let read = texture("r32float", json!([4, 4, 2]), STORAGE_BINDING, json!({}));
    let rw = texture("r32uint", json!([4, 4]), STORAGE_BINDING, json!({}));
    let view = |t: u32, d: serde_json::Value| WgpuTextureView::create(t, d).unwrap();
    let sampler = |d: serde_json::Value| WgpuSampler::create(device, d).unwrap();
    let (_, error) = validation_error(device, || {
        WgpuBindGroup::create(
            device,
            json!({ "layout": layout, "entries": [
                { "binding": 0, "resource": buffer(UNIFORM) },
                { "binding": 1, "resource": { "buffer": buffer(STORAGE), "offset": 0, "size": 64 } },
                { "binding": 2, "resource": { "buffer": WgpuBuffer::create(device, json!({ "size": 512, "usage": STORAGE })).unwrap(), "offset": 256 } },
                { "binding": 3, "resource": sampler(json!({ "magFilter": "linear", "minFilter": "linear", "mipmapFilter": "linear", "maxAnisotropy": 8 })) },
                { "binding": 4, "resource": sampler(json!({ "addressModeU": "repeat", "addressModeV": "mirror-repeat", "lodMinClamp": 1, "lodMaxClamp": 4 })) },
                { "binding": 5, "resource": sampler(json!({ "compare": "greater-equal" })) },
                { "binding": 6, "resource": float2d },
                { "binding": 7, "resource": view(layers, json!({ "dimension": "2d-array" })) },
                { "binding": 8, "resource": view(cube, json!({ "dimension": "cube" })) },
                { "binding": 9, "resource": volume },
                { "binding": 10, "resource": msaa },
                { "binding": 11, "resource": write },
                { "binding": 12, "resource": view(read, json!({ "dimension": "2d-array" })) },
                { "binding": 13, "resource": rw },
            ] }),
        )
        .unwrap()
    });
    assert_eq!(error, None);
}

#[test]
fn invalid_layouts_and_bind_groups_are_validation_errors() {
    let device = device();
    for (entries, why) in [
        (json!([{ "binding": 0, "visibility": 4 }]), "no member"),
        (
            json!([{ "binding": 0, "visibility": 4, "buffer": {}, "sampler": {} }]),
            "two members",
        ),
        (
            json!([{ "binding": 0, "visibility": 8, "buffer": {} }]),
            "visibility bit 8",
        ),
        (
            json!([{ "binding": 0, "visibility": 4, "buffer": {} }, { "binding": 0, "visibility": 4, "sampler": {} }]),
            "a binding twice",
        ),
        (
            json!([{ "binding": 0, "visibility": 1, "buffer": { "type": "storage" } }]),
            "writable storage in the vertex stage",
        ),
    ] {
        let (layout, error) = validation_error(device, || {
            WgpuBindGroupLayout::create(device, json!({ "entries": entries })).unwrap()
        });
        assert!(error.is_some(), "{why}");
        // What uses an invalid layout is invalid too.
        let (_, error) = validation_error(device, || {
            WgpuPipelineLayout::create(device, json!({ "bindGroupLayouts": [layout] })).unwrap()
        });
        assert!(error.is_some(), "pipeline layout of {why}");
    }
    assert_throws(
        WgpuBindGroupLayout::create(
            device,
            json!({ "entries": [{ "binding": 0, "visibility": 4, "buffer": { "type": "storage-ish" } }] }),
        ),
        "TypeError",
    );
    assert_throws(
        WgpuBindGroupLayout::create(
            device,
            json!({ "entries": [{ "binding": 0, "visibility": 4, "storageTexture": { "access": "atomic", "format": "r32uint" } }] }),
        ),
        "TypeError",
    );
    let layout = WgpuBindGroupLayout::create(
        device,
        json!({ "entries": [{ "binding": 0, "visibility": 4, "buffer": {} }] }),
    )
    .unwrap();
    let storage = WgpuBuffer::create(device, json!({ "size": 16, "usage": STORAGE })).unwrap();
    let uniform = WgpuBuffer::create(device, json!({ "size": 16, "usage": UNIFORM })).unwrap();
    for (entries, why) in [
        (
            json!([{ "binding": 0, "resource": storage }]),
            "a STORAGE buffer for a uniform binding",
        ),
        (
            json!([{ "binding": 1, "resource": uniform }]),
            "a binding the layout lacks",
        ),
        (json!([]), "a missing binding"),
        (
            json!([{ "binding": 0, "resource": { "buffer": uniform, "offset": 32 } }]),
            "an offset past the end",
        ),
        (
            json!([{ "binding": 0, "resource": { "buffer": uniform, "offset": 4 } }]),
            "a misaligned offset",
        ),
        (
            json!([{ "binding": 0, "resource": { "buffer": uniform, "size": 0 } }]),
            "a zero size",
        ),
    ] {
        let (_, error) = validation_error(device, || {
            WgpuBindGroup::create(device, json!({ "layout": layout, "entries": entries })).unwrap()
        });
        assert!(error.is_some(), "{why}");
    }
    // An object that is no binding resource is a TypeError.
    assert_throws(
        WgpuBindGroup::create(
            device,
            json!({ "layout": layout, "entries": [{ "binding": 0, "resource": layout }] }),
        ),
        "TypeError",
    );
}

/// ErrorScope.spec: an invalid maxAnisotropy is a validation error; the
/// spec's other sampler rules too.
#[test]
fn invalid_samplers_are_validation_errors() {
    let device = device();
    for (descriptor, valid) in [
        (json!(null), true),
        (
            json!({ "label": "s", "addressModeW": "repeat", "mipmapFilter": "linear" }),
            true,
        ),
        (
            json!({ "magFilter": "linear", "minFilter": "linear", "mipmapFilter": "linear", "maxAnisotropy": 100 }),
            true,
        ),
        (json!({ "maxAnisotropy": 0 }), false),
        (json!({ "maxAnisotropy": 2 }), false),
        (json!({ "lodMinClamp": -1 }), false),
        (json!({ "lodMinClamp": 4, "lodMaxClamp": 2 }), false),
    ] {
        let (sampler, error) = validation_error(device, || {
            WgpuSampler::create(device, descriptor.clone()).unwrap()
        });
        assert_eq!(error.is_none(), valid, "{descriptor}: {error:?}");
        assert_eq!(SamplerEntry::lookup(sampler).unwrap().valid, valid);
    }
    for descriptor in [
        json!({ "addressModeU": "clamp-to-border" }),
        json!({ "magFilter": "cubic" }),
        json!({ "compare": "sometimes" }),
    ] {
        assert_throws(WgpuSampler::create(device, descriptor), "TypeError");
    }
    // A bind group with an invalid sampler is invalid.
    let layout = WgpuBindGroupLayout::create(
        device,
        json!({ "entries": [{ "binding": 0, "visibility": 2, "sampler": {} }] }),
    )
    .unwrap();
    let (bad, _) = validation_error(device, || {
        WgpuSampler::create(device, json!({ "maxAnisotropy": 0 })).unwrap()
    });
    let (group, error) = validation_error(device, || {
        WgpuBindGroup::create(
            device,
            json!({ "layout": layout, "entries": [{ "binding": 0, "resource": bad }] }),
        )
        .unwrap()
    });
    assert!(error.is_some());
    assert!(!BindGroupEntry::lookup(group).unwrap().valid);
}

#[test]
fn compute_pipelines_validate_and_expose_their_layouts() {
    let device = device();
    let module = WgpuShaderModule::create(
        device,
        json!({ "code": "
          override scale: f32 = 1.0;
          @group(0) @binding(0) var<storage, read_write> out: array<f32>;
          @group(2) @binding(1) var<uniform> u: vec4f;
          @compute @workgroup_size(1) fn main() { out[0] = scale * u.x; }
          @compute @workgroup_size(1) fn other() {}" }),
    )
    .unwrap();
    // Two entry points: one must be named.
    let (_, error) = validation_error(device, || {
        WgpuComputePipeline::create(
            device,
            json!({ "layout": "auto", "compute": { "module": module } }),
        )
        .unwrap()
    });
    assert!(error.is_some(), "ambiguous entry point");
    let (_, error) = validation_error(device, || {
        WgpuComputePipeline::create(
            device,
            json!({ "layout": "auto", "compute": { "module": module, "entryPoint": "nope" } }),
        )
        .unwrap()
    });
    assert!(error.is_some(), "unknown entry point");
    let (_, error) = validation_error(device, || {
        WgpuComputePipeline::create(device, json!({ "layout": "auto", "compute": { "module": module, "entryPoint": "main", "constants": { "nope": 1 } } })).unwrap()
    });
    assert!(error.is_some(), "unknown constant");
    let (pipeline, error) = validation_error(device, || {
        WgpuComputePipeline::create(
            device,
            json!({ "label": "c", "layout": "auto", "compute": { "module": module, "entryPoint": "main", "constants": { "scale": 2 } } }),
        )
        .unwrap()
    });
    assert_eq!(error, None);
    // The auto layout's groups 0 and 2 (1 is empty).
    for index in [0, 1, 2] {
        let (_, error) = validation_error(device, || {
            WgpuComputePipeline::get_bind_group_layout(pipeline, index).unwrap()
        });
        assert_eq!(error, None, "group {index}");
    }
    let (_, error) = validation_error(device, || {
        WgpuComputePipeline::get_bind_group_layout(pipeline, 7).unwrap()
    });
    assert!(error.is_some(), "a group past maxBindGroups");
    // A layout that does not match the shader.
    let layout = WgpuBindGroupLayout::create(
        device,
        json!({ "entries": [{ "binding": 0, "visibility": FRAGMENT_STAGE, "buffer": {} }] }),
    )
    .unwrap();
    let pipeline_layout =
        WgpuPipelineLayout::create(device, json!({ "bindGroupLayouts": [layout] })).unwrap();
    let (_, error) = validation_error(device, || {
        WgpuComputePipeline::create(device, json!({ "layout": pipeline_layout, "compute": { "module": module, "entryPoint": "main" } })).unwrap()
    });
    assert!(error.is_some(), "mismatched layout");
    assert_throws(
        WgpuComputePipeline::create(
            device,
            json!({ "layout": "manual", "compute": { "module": module } }),
        ),
        "TypeError",
    );
    // Render pipelines share getBindGroupLayout.
    let render = WgpuShaderModule::create(
        device,
        json!({ "code": "
          @group(0) @binding(0) var<uniform> c: vec4f;
          @vertex fn vs() -> @builtin(position) vec4f { return c; }
          @fragment fn fs() -> @location(0) vec4f { return c; }" }),
    )
    .unwrap();
    let pipeline = WgpuRenderPipeline::create(
        device,
        json!({ "layout": "auto", "vertex": { "module": render }, "fragment": { "module": render, "targets": [{ "format": "rgba8unorm" }] } }),
    )
    .unwrap();
    let layout = WgpuRenderPipeline::get_bind_group_layout(pipeline, 0).unwrap();
    let uniform = WgpuBuffer::create(device, json!({ "size": 16, "usage": UNIFORM })).unwrap();
    let (_, error) = validation_error(device, || {
        WgpuBindGroup::create(device, json!({ "layout": layout, "entries": [{ "binding": 0, "resource": { "buffer": uniform } }] })).unwrap()
    });
    assert_eq!(error, None);
}
