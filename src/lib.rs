//! WebGPU for Native Shell (NSH) apps: a port of
//! [react-native-webgpu](https://github.com/wcandillon/react-native-webgpu)
//! on [wgpu](https://wgpu.rs).
//!
//! The crate is an nshm. Its native module (`nsh-webgpu/native`) is
//! handle-based: every WebGPU object is an id into a registry of wgpu
//! objects ([`registry`]), and one `#[module]` per spec interface
//! ([`api`]) takes ids, descriptors (plain objects) and bytes. The spec's
//! classes (`GPU`, `GPUDevice`, `GPUBuffer`, ...) are TSX, in `tsx/`,
//! exported as `nsh-webgpu` by `nsh.json`: they hold the ids and give the
//! browser's behaviour (`instanceof`, labels, getters, promises, events).
//! See `docs/architecture.md`.
//!
//! Rust code can call the same functions (`api::buffer::WgpuBuffer::create`).

pub mod api;
pub mod bytes;
pub mod convert;
pub mod error;
pub mod instance;
pub mod pending;
pub mod registry;
pub mod surface;

use nsh_core::prelude::*;

// The canvas view per platform: each `surface/<platform>.rs` defines its
// `#[native_components(<platform>)]` entry, which the hosts nsh builds find
// at the crate's root.
#[allow(unused_imports, unreachable_pub)]
pub use crate::surface::entries::*;

/// What TSX imports from `nsh-webgpu/native`: the native half of each
/// WebGPU interface, and the offscreen canvas's readback
/// (`surface::offscreen`).
#[native_module("nsh-webgpu/native")]
fn native(registry: &mut Registry) {
    use crate::api::{
        adapter::WgpuAdapter, bind_group::WgpuBindGroup, bind_group_layout::WgpuBindGroupLayout,
        buffer::WgpuBuffer, canvas_context::WgpuCanvasContext, command_encoder::WgpuCommandEncoder,
        compute_pass_encoder::WgpuComputePassEncoder, compute_pipeline::WgpuComputePipeline,
        device::WgpuDevice, external_texture::WgpuExternalTexture, gpu::Wgpu,
        image_bitmap::WgpuImageBitmap, pipeline_layout::WgpuPipelineLayout,
        query_set::WgpuQuerySet, queue::WgpuQueue, render_bundle::WgpuRenderBundle,
        render_bundle_encoder::WgpuRenderBundleEncoder, render_pass_encoder::WgpuRenderPassEncoder,
        render_pipeline::WgpuRenderPipeline, sampler::WgpuSampler, shader_module::WgpuShaderModule,
        texture::WgpuTexture, texture_view::WgpuTextureView,
    };
    use crate::surface::offscreen::WgpuOffscreenCanvas;
    registry
        .module::<Wgpu>()
        .module::<WgpuAdapter>()
        .module::<WgpuDevice>()
        .module::<WgpuQueue>()
        .module::<WgpuBuffer>()
        .module::<WgpuTexture>()
        .module::<WgpuTextureView>()
        .module::<WgpuSampler>()
        .module::<WgpuBindGroupLayout>()
        .module::<WgpuBindGroup>()
        .module::<WgpuPipelineLayout>()
        .module::<WgpuShaderModule>()
        .module::<WgpuRenderPipeline>()
        .module::<WgpuComputePipeline>()
        .module::<WgpuCommandEncoder>()
        .module::<WgpuRenderPassEncoder>()
        .module::<WgpuComputePassEncoder>()
        .module::<WgpuRenderBundleEncoder>()
        .module::<WgpuRenderBundle>()
        .module::<WgpuQuerySet>()
        .module::<WgpuExternalTexture>()
        .module::<WgpuImageBitmap>()
        .module::<WgpuCanvasContext>()
        .module::<WgpuOffscreenCanvas>();
}
