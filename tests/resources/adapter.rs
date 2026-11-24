//! `GPUSupportedFeatures`, `GPUSupportedLimits`, `GPUAdapterInfo`:
//! GPU.spec's features, limits and info, FeatureNames.spec's round trip.

use nsh_webgpu::api::{adapter::WgpuAdapter, device::WgpuDevice};
use serde_json::json;

use crate::common::{adapter, assert_throws, block};

/// The spec limits GPU.spec reads, and the newer ones.
const LIMITS: &[&str] = &[
    "maxTextureDimension1D",
    "maxTextureDimension2D",
    "maxTextureDimension3D",
    "maxTextureArrayLayers",
    "maxBindGroups",
    "maxBindGroupsPlusVertexBuffers",
    "maxBindingsPerBindGroup",
    "maxDynamicUniformBuffersPerPipelineLayout",
    "maxDynamicStorageBuffersPerPipelineLayout",
    "maxSampledTexturesPerShaderStage",
    "maxSamplersPerShaderStage",
    "maxStorageBuffersPerShaderStage",
    "maxStorageTexturesPerShaderStage",
    "maxUniformBuffersPerShaderStage",
    "maxUniformBufferBindingSize",
    "maxStorageBufferBindingSize",
    "minUniformBufferOffsetAlignment",
    "minStorageBufferOffsetAlignment",
    "maxVertexBuffers",
    "maxBufferSize",
    "maxVertexAttributes",
    "maxVertexBufferArrayStride",
    "maxInterStageShaderVariables",
    "maxColorAttachments",
    "maxColorAttachmentBytesPerSample",
    "maxComputeWorkgroupStorageSize",
    "maxComputeInvocationsPerWorkgroup",
    "maxComputeWorkgroupSizeX",
    "maxComputeWorkgroupSizeY",
    "maxComputeWorkgroupSizeZ",
    "maxComputeWorkgroupsPerDimension",
    "maxImmediateSize",
    "maxStorageBuffersInVertexStage",
    "maxStorageBuffersInFragmentStage",
    "maxStorageTexturesInVertexStage",
    "maxStorageTexturesInFragmentStage",
];

#[test]
fn adapter_features_are_spec_names_and_all_of_them_can_be_required() {
    let adapter = adapter();
    let features = WgpuAdapter::features(adapter).unwrap();
    eprintln!("adapter features: {features:?}");
    // GPU.spec "features".
    for name in [
        "depth-clip-control",
        "rg11b10ufloat-renderable",
        "core-features-and-limits",
    ] {
        assert!(features.contains(&name.to_owned()), "{name}");
    }
    // The reference asserts ETC2 (its Mac's GPU). The spec asks for BC, or
    // both ETC2 and ASTC: a virtualised Mac or a desktop GPU has only BC.
    let has = |name: &str| features.iter().any(|f| f == name);
    assert!(
        has("texture-compression-bc")
            || (has("texture-compression-etc2") && has("texture-compression-astc")),
        "{features:?}"
    );
    assert!(features.iter().all(|f| !f.is_empty()));
    // FeatureNames.spec: every advertised feature is accepted.
    let device = block(WgpuAdapter::request_device(
        adapter,
        json!({ "requiredFeatures": features }),
    ))
    .unwrap();
    assert_eq!(WgpuDevice::features(device).unwrap(), features);
    // A default device has only what every device has.
    let plain = block(WgpuAdapter::request_device(
        crate::common::adapter(),
        json!({}),
    ))
    .unwrap();
    assert_eq!(
        WgpuDevice::features(plain).unwrap(),
        vec!["core-features-and-limits"]
    );
    // A spec name wgpu cannot give, and a name that is none, reject.
    for name in ["texture-formats-tier2", "chromium-experimental-foo"] {
        let result = block(WgpuAdapter::request_device(
            crate::common::adapter(),
            json!({ "requiredFeatures": [name] }),
        ));
        assert_throws(result, "TypeError");
    }
}

#[test]
fn limits_have_every_spec_name_and_the_spec_bounds() {
    let adapter = adapter();
    let limits = WgpuAdapter::limits(adapter).unwrap();
    for name in LIMITS {
        assert!(limits[name].as_f64().unwrap() >= 0.0, "{name}");
    }
    assert_eq!(limits.as_object().unwrap().len(), LIMITS.len());
    let get = |n: &str| limits[n].as_f64().unwrap();
    // GPU.spec "limits".
    assert!(get("maxBindGroupsPlusVertexBuffers") > get("maxBindGroups"));
    assert_eq!(get("minStorageBufferOffsetAlignment").log2() % 1.0, 0.0);
    assert_eq!(get("minUniformBufferOffsetAlignment").log2() % 1.0, 0.0);
    assert!(get("maxTextureDimension1D") >= 2048.0);
    assert!(get("maxTextureDimension2D") >= 2048.0);
    assert!(get("maxTextureDimension3D") >= 256.0);
    // GPU.spec "requiredLimits" and "faulty input" (the limits part).
    let device = block(WgpuAdapter::request_device(
        adapter,
        json!({ "requiredLimits": {
            "maxBufferSize": 4 * 1024 * 1024,
            "maxComputeWorkgroupStorageSize": get("maxComputeWorkgroupStorageSize").min(16352.0),
            "maxComputeWorkgroupsPerDimension": get("maxComputeWorkgroupsPerDimension").min(65535.0),
            "maxStorageBufferBindingSize": get("maxStorageBufferBindingSize").min(268435456.0),
            "maxComputeWorkgroupSizeX": get("maxComputeWorkgroupSizeX").min(512.0),
            "maxComputeInvocationsPerWorkgroup": get("maxComputeInvocationsPerWorkgroup").min(512.0),
            "maxBindGroupsPlusVertexBuffers": 24,
        } }),
    ))
    .unwrap();
    let device_limits = WgpuDevice::limits(device).unwrap();
    // A required value below the default keeps the default.
    assert_eq!(
        device_limits["maxBufferSize"],
        json!(256.0 * 1024.0 * 1024.0)
    );
    assert_eq!(
        device_limits["maxComputeInvocationsPerWorkgroup"],
        json!(get("maxComputeInvocationsPerWorkgroup").min(512.0))
    );
    // Better than the adapter: OperationError; unknown name: OperationError.
    for limits in [
        json!({ "maxBindGroups": get("maxBindGroups") + 1.0 }),
        json!({ "maxFoo": 1 }),
    ] {
        assert_throws(
            block(WgpuAdapter::request_device(
                crate::common::adapter(),
                json!({ "requiredLimits": limits }),
            )),
            "OperationError",
        );
    }
}

#[test]
fn adapter_info_fields() {
    let info = WgpuAdapter::info(adapter()).unwrap();
    for key in [
        "vendor",
        "architecture",
        "device",
        "description",
        "subgroupMinSize",
        "subgroupMaxSize",
        "isFallbackAdapter",
    ] {
        assert!(info.get(key).is_some(), "{key}");
    }
    let software = [
        "swiftshader",
        "llvmpipe",
        "lavapipe",
        "microsoft basic render",
    ]
    .iter()
    .any(|s| {
        info["description"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains(s)
    });
    assert_eq!(info["isFallbackAdapter"], software);
}
