//! `GPUShaderModule`: `device.createShaderModule` (WGSL) and
//! `getCompilationInfo()`.

use nsh_core::{JsonValue, prelude::*};
use serde::Deserialize;
use serde_json::json;

use crate::{
    api::device::DeviceEntry,
    convert::parse,
    error::guard,
    registry::{self, Id},
};

/// A shader module in the registry.
#[derive(Clone)]
pub struct ShaderModuleEntry {
    /// The module.
    pub module: wgpu::ShaderModule,
    /// Its device.
    pub device: Id,
    /// Its WGSL, to count message positions in UTF-16 as the spec does.
    pub(crate) code: std::sync::Arc<str>,
}

impl ShaderModuleEntry {
    /// Module `id`'s entry, for Rust callers (`None`: no such module).
    pub fn lookup(id: u32) -> Option<Self> {
        registry::get::<ShaderModuleEntry>(id).ok()
    }
}

/// UTF-16 code units in `code[..byte]` (a byte offset clamped to the code
/// and to a character boundary).
fn utf16_units(code: &str, byte: usize) -> u64 {
    let mut byte = byte.min(code.len());
    while !code.is_char_boundary(byte) {
        byte -= 1;
    }
    code[..byte].encode_utf16().count() as u64
}

/// A naga location (1-based line, 1-based byte column, byte offset and
/// length) as the spec's `GPUCompilationMessage` counts it: line from 1,
/// position from 1, offset and length, the last three in UTF-16 code units.
fn spec_location(code: &str, l: &wgpu::SourceLocation) -> (u64, u64, u64, u64) {
    let offset = l.offset as usize;
    let end = offset + l.length as usize;
    let line_start = code[..offset.min(code.len())]
        .rfind('\n')
        .map_or(0, |i| i + 1);
    let offset16 = utf16_units(code, offset);
    let line_pos = offset16 - utf16_units(code, line_start) + 1;
    (
        l.line_number as u64,
        line_pos,
        offset16,
        utf16_units(code, end) - offset16,
    )
}

/// `GPUShaderModuleDescriptor` (`compilationHints` only help Dawn compile
/// ahead and `sourceMap` only helps tools; wgpu has no use for either, so
/// they are accepted and ignored, as the spec allows).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShaderModuleDescriptor {
    #[serde(default)]
    label: String,
    code: String,
    #[serde(default)]
    #[allow(dead_code)]
    compilation_hints: Vec<JsonValue>,
}

/// The native half of `GPUShaderModule`.
#[module]
pub struct WgpuShaderModule;

#[module]
impl WgpuShaderModule {
    /// `device.createShaderModule({ code })`: the module's id. WGSL that
    /// does not compile is a validation error and an invalid module, as in
    /// the spec; `getCompilationInfo` lists why.
    pub fn create(device: u32, descriptor: JsonValue) -> Result<u32, String> {
        guard(|| {
            let entry = DeviceEntry::get(device)?;
            let desc: ShaderModuleDescriptor = parse("GPUShaderModuleDescriptor", descriptor)?;
            let code: std::sync::Arc<str> = desc.code.into();
            let module = entry
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(desc.label.as_str()).filter(|l| !l.is_empty()),
                    source: wgpu::ShaderSource::Wgsl(code.as_ref().into()),
                });
            Ok(registry::insert(ShaderModuleEntry {
                module,
                device,
                code,
            }))
        })
    }

    /// `module.getCompilationInfo()`: the messages, each `{ message, type,
    /// lineNum, linePos, offset, length }`, positions in UTF-16 code units as
    /// the spec counts them (all 0 for a message without a location).
    pub fn compilation_info(module: u32) -> Result<Vec<JsonValue>, String> {
        guard(|| {
            let entry = registry::get::<ShaderModuleEntry>(module)?;
            let info = pollster::block_on(entry.module.get_compilation_info());
            Ok(info
                .messages
                .into_iter()
                .map(|m| {
                    let kind = match m.message_type {
                        wgpu::CompilationMessageType::Error => "error",
                        wgpu::CompilationMessageType::Warning => "warning",
                        wgpu::CompilationMessageType::Info => "info",
                    };
                    let (line, pos, offset, length) = m
                        .location
                        .map_or((0, 0, 0, 0), |l| spec_location(&entry.code, &l));
                    json!({
                        "message": m.message,
                        "type": kind,
                        "lineNum": line,
                        "linePos": pos,
                        "offset": offset,
                        "length": length,
                    })
                })
                .collect())
        })
    }
}
