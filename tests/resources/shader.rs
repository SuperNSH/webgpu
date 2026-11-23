//! `GPUShaderModule`: Shaders.spec, and compilation messages with the
//! spec's positions.

use nsh_webgpu::api::{device::WgpuDevice, shader_module::WgpuShaderModule};
use serde_json::json;

use crate::common::{assert_throws, device, validation_error};

const RED_FRAG_WGSL: &str = "@fragment
fn main() -> @location(0) vec4f {
  return vec4(1.0, 0.0, 0.0, 1.0);
}";

/// Shaders.spec "creates a shader module (3)": no messages for valid WGSL.
#[test]
fn valid_wgsl_has_no_messages() {
    let device = device();
    let module = WgpuShaderModule::create(
        device,
        json!({ "code": RED_FRAG_WGSL, "label": "redFragWGSL", "compilationHints": [{ "entryPoint": "main", "layout": "auto" }] }),
    )
    .unwrap();
    assert!(
        WgpuShaderModule::compilation_info(module)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn an_error_message_has_its_line_position_offset_and_length() {
    let device = device();
    // Line 3, after a line with non-ASCII text: positions count UTF-16
    // code units, not bytes.
    let code = "// héllo wörld 🎨\nfn f() -> f32 { return 1.0; }\nfn g() -> f32 { return nope; }\n";
    let (module, error) = validation_error(device, || {
        WgpuShaderModule::create(device, json!({ "code": code })).unwrap()
    });
    assert!(error.is_some());
    let messages = WgpuShaderModule::compilation_info(module).unwrap();
    let error = messages
        .iter()
        .find(|m| m["type"] == "error")
        .unwrap_or_else(|| panic!("{messages:?}"));
    assert!(
        error["message"].as_str().unwrap().contains("nope"),
        "{error}"
    );
    assert_eq!(error["lineNum"], 3, "{error}");
    let line_start16 = code
        .split('\n')
        .take(2)
        .map(|l| l.encode_utf16().count() + 1)
        .sum::<usize>();
    let nope16 = line_start16 + "fn g() -> f32 { return ".len();
    assert_eq!(error["offset"], nope16, "{error}");
    assert_eq!(
        error["linePos"],
        "fn g() -> f32 { return ".len() + 1,
        "{error}"
    );
    assert_eq!(error["length"], 4, "{error}");
    // In bytes the offset would be larger: the emoji is 4 bytes, 2 units.
    assert!(nope16 < code.find("nope").unwrap());
}

#[test]
fn a_parse_error_is_reported_with_a_location() {
    let device = device();
    let (module, _) = validation_error(device, || {
        WgpuShaderModule::create(device, json!({ "code": format!("{RED_FRAG_WGSL}  ++++ ") }))
            .unwrap()
    });
    let messages = WgpuShaderModule::compilation_info(module).unwrap();
    assert!(!messages.is_empty());
    assert!(messages.iter().all(|m| m["type"] == "error"));
    assert_eq!(messages[0]["lineNum"], 4, "{messages:?}");
    // A descriptor without code is a TypeError, no module.
    assert_throws(
        WgpuShaderModule::create(device, json!({ "label": "x" })),
        "TypeError",
    );
    // Nothing was left in a scope.
    assert_throws(WgpuDevice::pop_error_scope(device), "OperationError");
}
