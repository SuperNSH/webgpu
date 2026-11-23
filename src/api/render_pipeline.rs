//! `GPURenderPipeline`: `device.createRenderPipeline`,
//! `createRenderPipelineAsync` (compiled on a blocking thread) and
//! `getBindGroupLayout`.

use std::collections::BTreeMap;

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;
use serde_json::json;

#[cfg(target_arch = "wasm32")]
use crate::api::gpu::blocking;

use crate::{
    api::{
        bind_group_layout::BindGroupLayoutEntry, device::DeviceEntry,
        pipeline_layout::PipelineLayoutEntry, shader_module::ShaderModuleEntry,
    },
    convert::parse,
    error::{NativeError, guard, in_pipeline_scopes},
    registry::{self, Id},
};

/// A render pipeline in the registry.
#[derive(Clone)]
pub struct RenderPipelineEntry {
    /// The pipeline.
    pub pipeline: wgpu::RenderPipeline,
    /// Its device.
    pub device: Id,
}

impl RenderPipelineEntry {
    /// Pipeline `id`'s entry, for Rust callers (`None`: no such pipeline).
    pub fn lookup(id: u32) -> Option<Self> {
        registry::get::<RenderPipelineEntry>(id).ok()
    }
}

/// `layout`: `"auto"` or a pipeline layout's id. Shared with compute
/// pipelines.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum LayoutRef {
    /// `"auto"`.
    Auto(String),
    /// A `GPUPipelineLayout`.
    Layout(Id),
}

impl LayoutRef {
    /// The layout, `None` for `"auto"`.
    pub(crate) fn resolve(&self) -> Result<Option<wgpu::PipelineLayout>, NativeError> {
        match self {
            LayoutRef::Auto(s) if s == "auto" => Ok(None),
            LayoutRef::Auto(s) => Err(NativeError::type_error(format!(
                "layout: '{s}' is not \"auto\" or a GPUPipelineLayout"
            ))),
            LayoutRef::Layout(id) => Ok(Some(registry::get::<PipelineLayoutEntry>(*id)?.layout)),
        }
    }
}

/// `GPUProgrammableStage`'s members: module, entry point, constants.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Stage {
    pub(crate) module: Id,
    pub(crate) entry_point: Option<String>,
    #[serde(default)]
    pub(crate) constants: BTreeMap<String, f64>,
}

/// A stage's module and its pipeline-overridable constants.
pub(crate) type ResolvedStage<'a> = (wgpu::ShaderModule, Vec<(&'a str, f64)>);

impl Stage {
    /// The module and constants as wgpu takes them.
    pub(crate) fn resolve(&self) -> Result<ResolvedStage<'_>, NativeError> {
        let module = registry::get::<ShaderModuleEntry>(self.module)?.module;
        Ok((module, self.constants()))
    }

    /// The pipeline-overridable constants as wgpu takes them.
    pub(crate) fn constants(&self) -> Vec<(&str, f64)> {
        self.constants
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VertexAttribute {
    format: wgpu::VertexFormat,
    offset: u64,
    shader_location: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VertexBufferLayout {
    array_stride: u64,
    #[serde(default)]
    step_mode: wgpu::VertexStepMode,
    attributes: Vec<VertexAttribute>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VertexState {
    #[serde(flatten)]
    stage: Stage,
    #[serde(default)]
    buffers: Vec<Option<VertexBufferLayout>>,
}

/// `cullMode`: `"none"`, `"front"`, `"back"`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum CullMode {
    #[default]
    None,
    Front,
    Back,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrimitiveState {
    #[serde(default)]
    topology: wgpu::PrimitiveTopology,
    strip_index_format: Option<wgpu::IndexFormat>,
    #[serde(default)]
    front_face: wgpu::FrontFace,
    #[serde(default)]
    cull_mode: CullMode,
    #[serde(default)]
    unclipped_depth: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StencilFaceState {
    #[serde(default = "always")]
    compare: wgpu::CompareFunction,
    #[serde(default)]
    fail_op: wgpu::StencilOperation,
    #[serde(default)]
    depth_fail_op: wgpu::StencilOperation,
    #[serde(default)]
    pass_op: wgpu::StencilOperation,
}

impl Default for StencilFaceState {
    fn default() -> Self {
        Self {
            compare: wgpu::CompareFunction::Always,
            fail_op: Default::default(),
            depth_fail_op: Default::default(),
            pass_op: Default::default(),
        }
    }
}

fn always() -> wgpu::CompareFunction {
    wgpu::CompareFunction::Always
}

fn all_bits() -> u32 {
    u32::MAX
}

impl StencilFaceState {
    fn to_wgpu(&self) -> wgpu::StencilFaceState {
        wgpu::StencilFaceState {
            compare: self.compare,
            fail_op: self.fail_op,
            depth_fail_op: self.depth_fail_op,
            pass_op: self.pass_op,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DepthStencilState {
    format: wgpu::TextureFormat,
    depth_write_enabled: Option<bool>,
    depth_compare: Option<wgpu::CompareFunction>,
    #[serde(default)]
    stencil_front: StencilFaceState,
    #[serde(default)]
    stencil_back: StencilFaceState,
    #[serde(default = "all_bits")]
    stencil_read_mask: u32,
    #[serde(default = "all_bits")]
    stencil_write_mask: u32,
    #[serde(default)]
    depth_bias: i32,
    #[serde(default)]
    depth_bias_slope_scale: f32,
    #[serde(default)]
    depth_bias_clamp: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MultisampleState {
    #[serde(default = "one")]
    count: u32,
    #[serde(default = "all_bits")]
    mask: u32,
    #[serde(default)]
    alpha_to_coverage_enabled: bool,
}

impl Default for MultisampleState {
    fn default() -> Self {
        Self {
            count: 1,
            mask: u32::MAX,
            alpha_to_coverage_enabled: false,
        }
    }
}

fn one() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlendComponent {
    #[serde(default)]
    operation: wgpu::BlendOperation,
    #[serde(default = "blend_one")]
    src_factor: wgpu::BlendFactor,
    #[serde(default = "blend_zero")]
    dst_factor: wgpu::BlendFactor,
}

fn blend_one() -> wgpu::BlendFactor {
    wgpu::BlendFactor::One
}

fn blend_zero() -> wgpu::BlendFactor {
    wgpu::BlendFactor::Zero
}

impl BlendComponent {
    fn to_wgpu(&self) -> wgpu::BlendComponent {
        wgpu::BlendComponent {
            src_factor: self.src_factor,
            dst_factor: self.dst_factor,
            operation: self.operation,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlendState {
    color: BlendComponent,
    alpha: BlendComponent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ColorTargetState {
    format: wgpu::TextureFormat,
    blend: Option<BlendState>,
    #[serde(default = "all_colors")]
    write_mask: u32,
}

fn all_colors() -> u32 {
    0xf
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FragmentState {
    #[serde(flatten)]
    stage: Stage,
    targets: Vec<Option<ColorTargetState>>,
}

/// `GPURenderPipelineDescriptor`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPipelineDescriptor {
    #[serde(default)]
    label: String,
    layout: LayoutRef,
    vertex: VertexState,
    #[serde(default)]
    primitive: PrimitiveState,
    depth_stencil: Option<DepthStencilState>,
    #[serde(default)]
    multisample: MultisampleState,
    fragment: Option<FragmentState>,
}

/// A render pipeline's descriptor with the objects it names looked up:
/// what compiling it needs, `Send` to a blocking thread.
struct Prepared {
    device: wgpu::Device,
    desc: RenderPipelineDescriptor,
    layout: Option<wgpu::PipelineLayout>,
    vertex_module: wgpu::ShaderModule,
    fragment_module: Option<wgpu::ShaderModule>,
}

/// Reads `descriptor` for `device` (JS thread): a `TypeError` for one that
/// does not convert or names an object of the wrong kind.
fn prepare(device: u32, descriptor: JsonValue) -> Result<Prepared, NativeError> {
    let entry = DeviceEntry::get(device)?;
    let desc: RenderPipelineDescriptor = parse("GPURenderPipelineDescriptor", descriptor)?;
    let layout = desc.layout.resolve()?;
    let vertex_module = desc.vertex.stage.resolve()?.0;
    let fragment_module = match &desc.fragment {
        Some(f) => Some(f.stage.resolve()?.0),
        None => None,
    };
    Ok(Prepared {
        device: entry.device.clone(),
        desc,
        layout,
        vertex_module,
        fragment_module,
    })
}

impl Prepared {
    /// Compiles the pipeline (on any thread). An invalid descriptor is a
    /// validation error of the device and an invalid pipeline.
    fn build(&self) -> wgpu::RenderPipeline {
        let desc = &self.desc;
        let layout = &self.layout;
        let vertex_module = &self.vertex_module;
        let vertex_constants = desc.vertex.stage.constants();
        let attributes: Vec<Vec<wgpu::VertexAttribute>> = desc
            .vertex
            .buffers
            .iter()
            .map(|b| {
                b.as_ref().map_or_else(Vec::new, |b| {
                    b.attributes
                        .iter()
                        .map(|a| wgpu::VertexAttribute {
                            format: a.format,
                            offset: a.offset,
                            shader_location: a.shader_location,
                        })
                        .collect()
                })
            })
            .collect();
        // A hole in `buffers` is a slot with no attributes and stride 0.
        let buffers: Vec<wgpu::VertexBufferLayout<'_>> = desc
            .vertex
            .buffers
            .iter()
            .zip(&attributes)
            .map(|(b, attributes)| wgpu::VertexBufferLayout {
                array_stride: b.as_ref().map_or(0, |b| b.array_stride),
                step_mode: b
                    .as_ref()
                    .map_or(wgpu::VertexStepMode::Vertex, |b| b.step_mode),
                attributes,
            })
            .collect();
        let fragment = desc
            .fragment
            .as_ref()
            .zip(self.fragment_module.as_ref())
            .map(|(f, module)| ((module, f.stage.constants()), f));
        let targets: Vec<Option<wgpu::ColorTargetState>> = fragment
            .as_ref()
            .map(|(_, f)| {
                f.targets
                    .iter()
                    .map(|t| {
                        t.as_ref().map(|t| wgpu::ColorTargetState {
                            format: t.format,
                            blend: t.blend.as_ref().map(|b| wgpu::BlendState {
                                color: b.color.to_wgpu(),
                                alpha: b.alpha.to_wgpu(),
                            }),
                            write_mask: wgpu::ColorWrites::from_bits_truncate(t.write_mask),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let primitive = &desc.primitive;
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(desc.label.as_str()).filter(|l| !l.is_empty()),
                layout: layout.as_ref(),
                vertex: wgpu::VertexState {
                    module: vertex_module,
                    entry_point: desc.vertex.stage.entry_point.as_deref(),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &vertex_constants,
                        zero_initialize_workgroup_memory: true,
                    },
                    buffers: &buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: primitive.topology,
                    strip_index_format: primitive.strip_index_format,
                    front_face: primitive.front_face,
                    cull_mode: match primitive.cull_mode {
                        CullMode::None => None,
                        CullMode::Front => Some(wgpu::Face::Front),
                        CullMode::Back => Some(wgpu::Face::Back),
                    },
                    unclipped_depth: primitive.unclipped_depth,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: desc
                    .depth_stencil
                    .as_ref()
                    .map(|d| wgpu::DepthStencilState {
                        format: d.format,
                        depth_write_enabled: d.depth_write_enabled,
                        depth_compare: d.depth_compare,
                        stencil: wgpu::StencilState {
                            front: d.stencil_front.to_wgpu(),
                            back: d.stencil_back.to_wgpu(),
                            read_mask: d.stencil_read_mask,
                            write_mask: d.stencil_write_mask,
                        },
                        bias: wgpu::DepthBiasState {
                            constant: d.depth_bias,
                            slope_scale: d.depth_bias_slope_scale,
                            clamp: d.depth_bias_clamp,
                        },
                    }),
                multisample: wgpu::MultisampleState {
                    count: desc.multisample.count,
                    mask: desc.multisample.mask as u64,
                    alpha_to_coverage_enabled: desc.multisample.alpha_to_coverage_enabled,
                },
                fragment: fragment
                    .as_ref()
                    .map(|((module, constants), f)| wgpu::FragmentState {
                        module,
                        entry_point: f.stage.entry_point.as_deref(),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants,
                            zero_initialize_workgroup_memory: true,
                        },
                        targets: &targets,
                    }),
                multiview_mask: None,
                cache: None,
            })
    }
}

/// The native half of `GPURenderPipeline`.
#[module]
pub struct WgpuRenderPipeline;

#[module]
impl WgpuRenderPipeline {
    /// `device.createRenderPipeline(descriptor)`: the pipeline's id. An
    /// invalid descriptor is a validation error and an invalid pipeline.
    pub fn create(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let prepared = prepare(device, descriptor)?;
            let pipeline = prepared.build();
            Ok(registry::insert(RenderPipelineEntry { pipeline, device }))
        })
    }

    /// `device.createRenderPipelineAsync(descriptor)`: the descriptor is
    /// read on the JS thread (a `TypeError` rejects), the pipeline compiled
    /// on a blocking thread in error scopes of its own, so the JS thread
    /// keeps running meanwhile. `{ id }`, or `{ error: { reason, message } }`
    /// for an invalid pipeline (TSX rejects with a `GPUPipelineError`; no
    /// error reaches the device's scopes or `uncapturederror`).
    pub async fn create_async(device: u32, descriptor: JsonValue) -> Result<JsonValue, String> {
        let prepared = guard(|| prepare(device, descriptor))?;
        let compiled = blocking(move || in_pipeline_scopes(&prepared.device, || prepared.build()))
            .await
            .map_err(|e| NativeError::operation(format!("createRenderPipelineAsync: {e}")))?;
        Ok(match compiled {
            Ok(pipeline) => {
                json!({ "id": registry::insert(RenderPipelineEntry { pipeline, device }) })
            }
            Err(error) => error.to_json(),
        })
    }

    /// `pipeline.getBindGroupLayout(index)`: the layout's id (a new id each
    /// call, as a new object each call in Dawn).
    pub fn get_bind_group_layout(pipeline: u32, index: u32) -> Result<u32, String> {
        guard(|| {
            let entry = registry::get::<RenderPipelineEntry>(pipeline)?;
            let layout = entry.pipeline.get_bind_group_layout(index);
            Ok(registry::insert(BindGroupLayoutEntry {
                layout,
                device: entry.device,
                valid: true,
            }))
        })
    }
}
