//! `GPUComputePipeline`: `device.createComputePipeline`,
//! `createComputePipelineAsync` (compiled on a blocking thread, as
//! `createRenderPipelineAsync` is) and `getBindGroupLayout`.

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;
use serde_json::json;

#[cfg(target_arch = "wasm32")]
use crate::api::gpu::blocking;

use crate::{
    api::{
        bind_group_layout::BindGroupLayoutEntry,
        device::DeviceEntry,
        render_pipeline::{LayoutRef, Stage},
    },
    convert::parse,
    error::{NativeError, guard, in_pipeline_scopes},
    registry::{self, Id},
};

/// A compute pipeline in the registry.
#[derive(Clone)]
pub struct ComputePipelineEntry {
    /// The pipeline.
    pub pipeline: wgpu::ComputePipeline,
    /// Its device.
    pub device: Id,
}

impl ComputePipelineEntry {
    /// Pipeline `id`'s entry, for Rust callers (`None`: no such pipeline).
    pub fn lookup(id: u32) -> Option<Self> {
        registry::get::<ComputePipelineEntry>(id).ok()
    }
}

/// `GPUComputePipelineDescriptor`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePipelineDescriptor {
    #[serde(default)]
    label: String,
    layout: LayoutRef,
    compute: Stage,
}

/// A compute pipeline's descriptor with the objects it names looked up,
/// `Send` to a blocking thread.
struct Prepared {
    device: wgpu::Device,
    desc: ComputePipelineDescriptor,
    layout: Option<wgpu::PipelineLayout>,
    module: wgpu::ShaderModule,
}

/// Reads `descriptor` for `device` (JS thread).
fn prepare(device: u32, descriptor: JsonValue) -> Result<Prepared, NativeError> {
    let entry = DeviceEntry::get(device)?;
    let desc: ComputePipelineDescriptor = parse("GPUComputePipelineDescriptor", descriptor)?;
    let layout = desc.layout.resolve()?;
    let module = desc.compute.resolve()?.0;
    Ok(Prepared {
        device: entry.device.clone(),
        desc,
        layout,
        module,
    })
}

impl Prepared {
    /// Compiles the pipeline (on any thread). An invalid descriptor (a
    /// missing entry point, a layout that does not match the shader, an
    /// unknown override constant) is a validation error and an invalid
    /// pipeline.
    fn build(&self) -> wgpu::ComputePipeline {
        let constants = self.desc.compute.constants();
        self.device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(self.desc.label.as_str()).filter(|l| !l.is_empty()),
                layout: self.layout.as_ref(),
                module: &self.module,
                entry_point: self.desc.compute.entry_point.as_deref(),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &constants,
                    zero_initialize_workgroup_memory: true,
                },
                cache: None,
            })
    }
}

/// The native half of `GPUComputePipeline`.
#[module]
pub struct WgpuComputePipeline;

#[module]
impl WgpuComputePipeline {
    /// `device.createComputePipeline(descriptor)`: the pipeline's id. An
    /// invalid descriptor (a missing entry point, a layout that does not
    /// match the shader, an unknown override constant) is a validation
    /// error and an invalid pipeline.
    pub fn create(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let pipeline = prepare(device, descriptor)?.build();
            Ok(registry::insert(ComputePipelineEntry { pipeline, device }))
        })
    }

    /// `device.createComputePipelineAsync(descriptor)`: read on the JS
    /// thread (a `TypeError` rejects), compiled on a blocking thread in
    /// error scopes of its own. `{ id }`, or `{ error: { reason, message } }`
    /// (TSX rejects with a `GPUPipelineError`).
    pub async fn create_async(device: u32, descriptor: JsonValue) -> Result<JsonValue, String> {
        let prepared = guard(|| prepare(device, descriptor))?;
        let compiled = blocking(move || in_pipeline_scopes(&prepared.device, || prepared.build()))
            .await
            .map_err(|e| NativeError::operation(format!("createComputePipelineAsync: {e}")))?;
        Ok(match compiled {
            Ok(pipeline) => {
                json!({ "id": registry::insert(ComputePipelineEntry { pipeline, device }) })
            }
            Err(error) => error.to_json(),
        })
    }

    /// `pipeline.getBindGroupLayout(index)`: the layout's id (a new id each
    /// call). An index past the layout is a validation error.
    pub fn get_bind_group_layout(pipeline: u32, index: u32) -> Result<u32, String> {
        guard(|| {
            let entry = registry::get::<ComputePipelineEntry>(pipeline)?;
            let layout = entry.pipeline.get_bind_group_layout(index);
            Ok(registry::insert(BindGroupLayoutEntry {
                layout,
                device: entry.device,
                valid: true,
            }))
        })
    }
}
