//! `GPUAdapter`: its info, features and limits, and `requestDevice`.

use std::collections::BTreeMap;

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;
use serde_json::json;

#[cfg(target_arch = "wasm32")]
use crate::api::gpu::blocking;
use crate::{
    api::{device::DeviceEntry, features, limits},
    convert::parse,
    error::{NativeError, guard},
    registry::{self, Id},
};

/// An adapter in the registry.
#[derive(Clone)]
pub struct AdapterEntry {
    /// The adapter.
    pub adapter: wgpu::Adapter,
    /// Whether `forceFallbackAdapter` asked for it.
    pub fallback: bool,
}

/// Whether an adapter gave its device: the spec's "consumed" state, a
/// second `requestDevice` rejects. Kept apart from the cloneable entry.
static CONSUMED: std::sync::Mutex<std::collections::BTreeSet<Id>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// `GPUDeviceDescriptor`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceDescriptor {
    #[serde(default)]
    label: String,
    #[serde(default)]
    required_features: Vec<String>,
    #[serde(default)]
    required_limits: BTreeMap<String, f64>,
}

/// The spec's vendor name of PCI vendor `id` (`""` when unknown).
fn vendor_name(info: &wgpu::AdapterInfo) -> String {
    let by_id = match info.vendor {
        0x106b => "apple",
        0x10de => "nvidia",
        0x1002 | 0x1022 => "amd",
        0x8086 => "intel",
        0x13b5 => "arm",
        0x5143 => "qualcomm",
        0x1010 => "img-tec",
        0x14e4 => "broadcom",
        0x1af4 => "red-hat",
        0x10005 => "mesa",
        0x1414 => "microsoft",
        0x15ad => "vmware",
        _ => "",
    };
    if !by_id.is_empty() {
        return by_id.into();
    }
    // Metal reports no PCI id; its adapters name the maker.
    if info.backend == wgpu::Backend::Metal && info.name.starts_with("Apple") {
        return "apple".into();
    }
    String::new()
}

/// The native half of `GPUAdapter`.
#[module]
pub struct WgpuAdapter;

#[module]
impl WgpuAdapter {
    /// `adapter.info`: `{ vendor, architecture, device, description,
    /// subgroupMinSize, subgroupMaxSize, isFallbackAdapter }`.
    pub fn info(adapter: u32) -> Result<JsonValue, String> {
        guard(|| {
            let entry = registry::get::<AdapterEntry>(adapter)?;
            let info = entry.adapter.get_info();
            Ok(json!({
                "vendor": vendor_name(&info),
                "architecture": "",
                "device": if info.device == 0 { String::new() } else { format!("0x{:x}", info.device) },
                "description": info.name,
                "subgroupMinSize": info.subgroup_min_size,
                "subgroupMaxSize": info.subgroup_max_size,
                "isFallbackAdapter": entry.fallback || info.device_type == wgpu::DeviceType::Cpu,
            }))
        })
    }

    /// `adapter.features`: the WebGPU names of the features it has.
    pub fn features(adapter: u32) -> Result<Vec<String>, String> {
        guard(|| {
            Ok(features::names(
                registry::get::<AdapterEntry>(adapter)?.adapter.features(),
            ))
        })
    }

    /// `adapter.limits`: the spec's limits, by name.
    pub fn limits(adapter: u32) -> Result<JsonValue, String> {
        guard(|| {
            let limits = registry::get::<AdapterEntry>(adapter)?.adapter.limits();
            Ok(json!(limits::to_map(&limits)))
        })
    }

    /// `adapter.requestDevice(descriptor)`: the device's id (its queue is
    /// the device's). Rejects with a `TypeError` for a feature the adapter
    /// lacks, an `OperationError` for a limit it cannot give or a second
    /// request.
    pub async fn request_device(adapter: u32, descriptor: JsonValue) -> Result<u32, String> {
        let entry = registry::get::<AdapterEntry>(adapter).map_err(String::from)?;
        let desc: DeviceDescriptor =
            parse("GPUDeviceDescriptor", descriptor).map_err(String::from)?;
        let required_features = features::parse(&desc.required_features).map_err(String::from)?;
        let missing = required_features - entry.adapter.features();
        if !missing.is_empty() {
            return Err(NativeError::type_error(format!(
                "requestDevice: the adapter does not support {:?}",
                features::names(missing)
            ))
            .into());
        }
        // The spec's defaults, raised where asked; wgpu checks them against
        // the adapter's.
        // An adapter below the spec's defaults (the iOS simulator's
        // maxInterStageShaderVariables is 15) gives what it has rather than
        // no device at all.
        let base = wgpu::Limits::default().or_worse_values_from(&entry.adapter.limits());
        let required_limits = limits::apply(base, &desc.required_limits).map_err(String::from)?;
        if !CONSUMED
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(adapter)
        {
            return Err(NativeError::operation(
                "requestDevice: this adapter already gave its device (it is consumed)",
            )
            .into());
        }
        let label = desc.label;
        let adapter_handle = entry.adapter.clone();
        let adapter_kept = entry.adapter.clone();
        // Not a WebGPU feature name: wgpu's external textures back the spec's
        // `externalTexture` binding, so take them whenever the adapter has them.
        let required_features =
            required_features | (adapter_handle.features() & wgpu::Features::EXTERNAL_TEXTURE);
        let (device, queue) = blocking(move || {
            let desc = wgpu::DeviceDescriptor {
                label: Some(label.as_str()).filter(|l| !l.is_empty()),
                required_features,
                required_limits,
                ..Default::default()
            };
            pollster::block_on(adapter_handle.request_device(&desc))
        })
        .await
        .map_err(|e| NativeError::operation(format!("requestDevice: {e}")))?
        .map_err(|e| NativeError::operation(format!("requestDevice: {e}")))?;
        Ok(DeviceEntry::register(device, queue, Some(adapter_kept)))
    }
}
