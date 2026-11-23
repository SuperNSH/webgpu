//! The spec's interfaces, one file each: the native half of `GPUBuffer` is
//! `buffer.rs` (`#[module] WgpuBuffer`), its TSX half
//! `tsx/api/GPUBuffer.ts`. A file owns its kind's registry entry type and
//! the device's `create*` for it (`WgpuBuffer.create`), so the areas of
//! phase 2 never edit one file together. See docs/architecture.md.

pub mod adapter;
pub mod bind_group;
pub mod bind_group_layout;
pub mod buffer;
pub mod canvas_context;
pub mod command_buffer;
pub mod command_encoder;
pub mod compute_pass_encoder;
pub mod compute_pipeline;
pub mod device;
pub mod external_texture;
pub(crate) mod features;
pub mod gpu;
pub mod image_bitmap;
pub(crate) mod limits;
pub mod pipeline_layout;
pub mod query_set;
pub mod queue;
pub mod render_bundle;
pub mod render_bundle_encoder;
pub mod render_pass_encoder;
pub mod render_pipeline;
pub mod sampler;
pub mod shader_module;
pub mod texture;
pub mod texture_view;
