//! `GPUQuerySet`: occlusion and timestamp sets, their limits and
//! `destroy()`. The occlusion query runs through wgpu directly (the pass
//! commands are another area's).

use nsh_webgpu::api::{
    adapter::WgpuAdapter,
    buffer::{BufferEntry, WgpuBuffer},
    query_set::{QuerySetEntry, WgpuQuerySet},
    render_pipeline::{RenderPipelineEntry, WgpuRenderPipeline},
    shader_module::WgpuShaderModule,
};
use serde_json::json;

use crate::common::{adapter, assert_throws, block, device, submit, validation_error, wait};

#[test]
fn query_sets_and_their_validation() {
    let device = device();
    let (set, error) = validation_error(device, || {
        WgpuQuerySet::create(
            device,
            json!({ "type": "occlusion", "count": 2, "label": "q" }),
        )
        .unwrap()
    });
    assert_eq!(error, None);
    let entry = QuerySetEntry::lookup(set).unwrap();
    assert!(matches!(entry.ty, wgpu::QueryType::Occlusion));
    assert_eq!((entry.count, entry.valid), (2, true));
    let (big, error) = validation_error(device, || {
        WgpuQuerySet::create(device, json!({ "type": "occlusion", "count": 4097 })).unwrap()
    });
    assert!(error.is_some());
    assert!(!QuerySetEntry::lookup(big).unwrap().valid);
    // This device asked for no features: timestamps are invalid.
    let (_, error) = validation_error(device, || {
        WgpuQuerySet::create(device, json!({ "type": "timestamp", "count": 2 })).unwrap()
    });
    assert!(error.unwrap().contains("timestamp-query"));
    assert_throws(
        WgpuQuerySet::create(device, json!({ "type": "pipeline-statistics", "count": 1 })),
        "TypeError",
    );
    assert_throws(
        WgpuQuerySet::create(
            device,
            json!({ "type": "occlusion", "count": 4294967296u64 }),
        ),
        "TypeError",
    );
    assert!(!entry.destroyed());
    WgpuQuerySet::destroy(set).unwrap();
    WgpuQuerySet::destroy(set).unwrap();
    assert!(QuerySetEntry::lookup(set).unwrap().destroyed());
}

#[test]
fn timestamp_sets_where_the_adapter_has_the_feature() {
    let adapter = adapter();
    if !WgpuAdapter::features(adapter)
        .unwrap()
        .contains(&"timestamp-query".to_owned())
    {
        eprintln!("timestamp-query is not supported, skipping");
        return;
    }
    let device = block(WgpuAdapter::request_device(
        adapter,
        json!({ "requiredFeatures": ["timestamp-query"] }),
    ))
    .unwrap();
    let (_, error) = validation_error(device, || {
        WgpuQuerySet::create(device, json!({ "type": "timestamp", "count": 8 })).unwrap()
    });
    assert_eq!(error, None);
}

/// An occlusion query counts the triangle's samples.
#[test]
fn an_occlusion_query_counts_samples() {
    let device = device();
    let module = WgpuShaderModule::create(
        device,
        json!({ "code": "
          @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
            var p = array<vec2f, 3>(vec2(0.0, 0.5), vec2(-0.5, -0.5), vec2(0.5, -0.5));
            return vec4f(p[i], 0.0, 1.0);
          }
          @fragment fn fs() -> @location(0) vec4f { return vec4f(1.0); }" }),
    )
    .unwrap();
    let pipeline = WgpuRenderPipeline::create(
        device,
        json!({ "layout": "auto", "vertex": { "module": module }, "fragment": { "module": module, "targets": [{ "format": "rgba8unorm" }] } }),
    )
    .unwrap();
    let set = WgpuQuerySet::create(device, json!({ "type": "occlusion", "count": 2 })).unwrap();
    // QUERY_RESOLVE | COPY_SRC, and a readback.
    let resolved = WgpuBuffer::create(device, json!({ "size": 16, "usage": 0x200 | 0x4 })).unwrap();
    let readback = WgpuBuffer::create(device, json!({ "size": 16, "usage": 0x1 | 0x8 })).unwrap();
    let (d, _) = crate::common::wgpu(device);
    let target = d.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let (set_entry, pipeline) = (
        QuerySetEntry::lookup(set).unwrap(),
        RenderPipelineEntry::lookup(pipeline).unwrap(),
    );
    let (resolved_buffer, readback_buffer) = (
        BufferEntry::lookup(resolved).unwrap().buffer,
        BufferEntry::lookup(readback).unwrap().buffer,
    );
    submit(device, |e| {
        {
            let mut pass = e.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: Some(&set_entry.query_set),
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline.pipeline);
            pass.begin_occlusion_query(0);
            pass.draw(0..3, 0..1);
            pass.end_occlusion_query();
            pass.begin_occlusion_query(1);
            pass.end_occlusion_query();
        }
        e.resolve_query_set(&set_entry.query_set, 0..2, &resolved_buffer, 0);
        e.copy_buffer_to_buffer(&resolved_buffer, 0, &readback_buffer, 0, 16);
    });
    wait(WgpuBuffer::map_async(readback, 1, None, None).unwrap()).unwrap();
    let bytes = WgpuBuffer::get_mapped_range(readback, None, None).unwrap();
    let drawn = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let empty = u64::from_le_bytes(bytes[8..].try_into().unwrap());
    assert!(drawn > 0, "{drawn}");
    assert_eq!(empty, 0);
}
