//! `GPUSupportedLimits`: wgpu's limits by their WebGPU names. The table
//! lists the limits the spec defines, in its order; wgpu's other limits
//! (mesh shaders, ray tracing, binding arrays) have no spec name.
//!
//! Three kinds of spec limit have no wgpu field of their own:
//!
//! - `maxBindGroupsPlusVertexBuffers`: wgpu enforces bind groups and vertex
//!   buffers apart, so the combined limit is their sum.
//! - `max{Storage{Buffers,Textures}}In{Vertex,Fragment}Stage`: wgpu's
//!   per-shader-stage limit applies to each stage, so each is the matching
//!   `max*PerShaderStage`.
//!
//! `requestDevice`'s `requiredLimits` accepts them (a value the adapter
//! cannot give is an `OperationError`) and changes nothing: the device's
//! own limits already give them.

use std::collections::BTreeMap;

use crate::error::NativeError;

/// Whether a limit is a minimum (an alignment: smaller is better) rather
/// than a maximum.
fn is_alignment(name: &str) -> bool {
    name.starts_with("min")
}

macro_rules! limits {
    ($($name:literal => $field:ident),* $(,)?) => {
        /// The spec's limits of `limits`, by name.
        pub(crate) fn to_map(limits: &wgpu::Limits) -> BTreeMap<String, f64> {
            let mut map = BTreeMap::from([$(($name.to_owned(), limits.$field as f64)),*]);
            for (name, value) in derived(limits) {
                map.insert(name.to_owned(), value);
            }
            map
        }

        /// `base` with the `required` limits set (`requestDevice`'s
        /// `requiredLimits`; TSX drops `undefined` members first). As the
        /// spec says: a value worse than the default keeps the default; an
        /// alignment that is not a power of two, or a name the spec does
        /// not define, is an `OperationError`; a negative or non-finite
        /// value a `TypeError` (`[EnforceRange] GPUSize64`). A value better
        /// than the adapter's is wgpu's to refuse (`OperationError`).
        pub(crate) fn apply(
            mut base: wgpu::Limits,
            required: &BTreeMap<String, f64>,
        ) -> Result<wgpu::Limits, NativeError> {
            for (name, value) in required {
                if !value.is_finite() || *value < 0.0 {
                    return Err(NativeError::type_error(format!(
                        "requiredLimits: {name} = {value} is not a GPUSize64"
                    )));
                }
                let value = value.trunc();
                if is_alignment(name) && !(value as u64).is_power_of_two() {
                    return Err(NativeError::operation(format!(
                        "requiredLimits: {name} = {value} is not a power of two"
                    )));
                }
                match name.as_str() {
                    $($name => {
                        let current = base.$field as f64;
                        let better = if is_alignment($name) {
                            value.min(current)
                        } else {
                            value.max(current)
                        };
                        base.$field = better as _;
                    })*
                    other if DERIVED.contains(&other) => {}
                    other => {
                        return Err(NativeError::operation(format!(
                            "requiredLimits: '{other}' is not a limit this adapter knows"
                        )));
                    }
                }
            }
            Ok(base)
        }
    };
}

limits! {
    "maxTextureDimension1D" => max_texture_dimension_1d,
    "maxTextureDimension2D" => max_texture_dimension_2d,
    "maxTextureDimension3D" => max_texture_dimension_3d,
    "maxTextureArrayLayers" => max_texture_array_layers,
    "maxBindGroups" => max_bind_groups,
    "maxBindingsPerBindGroup" => max_bindings_per_bind_group,
    "maxDynamicUniformBuffersPerPipelineLayout" => max_dynamic_uniform_buffers_per_pipeline_layout,
    "maxDynamicStorageBuffersPerPipelineLayout" => max_dynamic_storage_buffers_per_pipeline_layout,
    "maxSampledTexturesPerShaderStage" => max_sampled_textures_per_shader_stage,
    "maxSamplersPerShaderStage" => max_samplers_per_shader_stage,
    "maxStorageBuffersPerShaderStage" => max_storage_buffers_per_shader_stage,
    "maxStorageTexturesPerShaderStage" => max_storage_textures_per_shader_stage,
    "maxUniformBuffersPerShaderStage" => max_uniform_buffers_per_shader_stage,
    "maxUniformBufferBindingSize" => max_uniform_buffer_binding_size,
    "maxStorageBufferBindingSize" => max_storage_buffer_binding_size,
    "minUniformBufferOffsetAlignment" => min_uniform_buffer_offset_alignment,
    "minStorageBufferOffsetAlignment" => min_storage_buffer_offset_alignment,
    "maxVertexBuffers" => max_vertex_buffers,
    "maxBufferSize" => max_buffer_size,
    "maxVertexAttributes" => max_vertex_attributes,
    "maxVertexBufferArrayStride" => max_vertex_buffer_array_stride,
    "maxInterStageShaderVariables" => max_inter_stage_shader_variables,
    "maxColorAttachments" => max_color_attachments,
    "maxColorAttachmentBytesPerSample" => max_color_attachment_bytes_per_sample,
    "maxComputeWorkgroupStorageSize" => max_compute_workgroup_storage_size,
    "maxComputeInvocationsPerWorkgroup" => max_compute_invocations_per_workgroup,
    "maxComputeWorkgroupSizeX" => max_compute_workgroup_size_x,
    "maxComputeWorkgroupSizeY" => max_compute_workgroup_size_y,
    "maxComputeWorkgroupSizeZ" => max_compute_workgroup_size_z,
    "maxComputeWorkgroupsPerDimension" => max_compute_workgroups_per_dimension,
    "maxImmediateSize" => max_immediate_size,
}

/// The spec limits wgpu has no field for (module docs).
pub(crate) const DERIVED: &[&str] = &[
    "maxBindGroupsPlusVertexBuffers",
    "maxStorageBuffersInVertexStage",
    "maxStorageBuffersInFragmentStage",
    "maxStorageTexturesInVertexStage",
    "maxStorageTexturesInFragmentStage",
];

/// The values of [`DERIVED`] for `limits`.
fn derived(limits: &wgpu::Limits) -> [(&'static str, f64); 5] {
    let buffers = limits.max_storage_buffers_per_shader_stage as f64;
    let textures = limits.max_storage_textures_per_shader_stage as f64;
    [
        (
            DERIVED[0],
            (limits.max_bind_groups + limits.max_vertex_buffers) as f64,
        ),
        (DERIVED[1], buffers),
        (DERIVED[2], buffers),
        (DERIVED[3], textures),
        (DERIVED[4], textures),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spec_limit_has_a_value() {
        let map = to_map(&wgpu::Limits::default());
        // The 31 of GPUSupportedLimits in react-native-webgpu, maxImmediateSize
        // and the four per-stage storage limits.
        assert_eq!(map.len(), 36);
        assert_eq!(map["maxBindGroupsPlusVertexBuffers"], 12.0);
        assert_eq!(map["maxStorageBuffersInVertexStage"], 8.0);
    }

    #[test]
    fn required_limits_keep_the_better_value() {
        let base = wgpu::Limits::default();
        let required = BTreeMap::from([
            ("maxBindGroups".to_owned(), 1.0),
            ("maxBufferSize".to_owned(), 4.0 * 1024.0 * 1024.0 * 1024.0),
            ("minUniformBufferOffsetAlignment".to_owned(), 64.0),
            ("maxBindGroupsPlusVertexBuffers".to_owned(), 24.0),
        ]);
        let limits = apply(base.clone(), &required).unwrap();
        assert_eq!(limits.max_bind_groups, base.max_bind_groups);
        assert_eq!(limits.max_buffer_size, 4 * 1024 * 1024 * 1024);
        assert_eq!(limits.min_uniform_buffer_offset_alignment, 64);
        let odd = BTreeMap::from([("minStorageBufferOffsetAlignment".to_owned(), 48.0)]);
        assert!(
            apply(base.clone(), &odd)
                .unwrap_err()
                .to_string()
                .starts_with("OperationError")
        );
        let negative = BTreeMap::from([("maxBindGroups".to_owned(), -1.0)]);
        assert!(
            apply(base, &negative)
                .unwrap_err()
                .to_string()
                .starts_with("TypeError")
        );
    }
}
