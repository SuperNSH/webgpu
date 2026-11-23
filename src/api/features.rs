//! `GPUSupportedFeatures` / `GPUFeatureName`: wgpu's features by their
//! WebGPU names.
//!
//! Reported: the features wgpu files under WebGPU
//! (`Features::all_webgpu_mask`) except `immediates`, which the spec does
//! not name yet; `subgroups` (wgpu's `SUBGROUP`, a native flag that has the
//! spec's name); `texture-formats-tier1` from wgpu's
//! `TEXTURE_FORMAT_16BIT_NORM` (the 16-bit normalized formats, what the tier
//! adds that wgpu gates); and `core-features-and-limits`, always (a wgpu
//! device is a core device, never a compatibility one). wgpu's other native
//! features have no spec name and stay out, as Dawn's do in a browser, and
//! no feature is ever reported as `""`.
//!
//! The spec names wgpu cannot give (`texture-formats-tier2`,
//! `texture-component-swizzle`, `subgroup-size-control`,
//! `texture-compression-unaligned`, `atomic-vec2u-min-max`) are never
//! reported; asking for one is the spec's `TypeError` for a feature the
//! adapter does not have.

use crate::error::NativeError;

/// The spec's feature names, as `GPUFeatureName` lists them.
pub(crate) const SPEC_NAMES: &[&str] = &[
    "core-features-and-limits",
    "depth-clip-control",
    "depth32float-stencil8",
    "texture-compression-bc",
    "texture-compression-bc-sliced-3d",
    "texture-compression-etc2",
    "texture-compression-astc",
    "texture-compression-astc-sliced-3d",
    "timestamp-query",
    "indirect-first-instance",
    "shader-f16",
    "rg11b10ufloat-renderable",
    "bgra8unorm-storage",
    "float32-filterable",
    "float32-blendable",
    "clip-distances",
    "dual-source-blending",
    "subgroups",
    "texture-formats-tier1",
    "texture-formats-tier2",
    "primitive-index",
    "texture-component-swizzle",
    "subgroup-size-control",
    "texture-compression-unaligned",
    "atomic-vec2u-min-max",
];

/// The wgpu features of spec name `name`: `Some(empty)` for a name every
/// device has (`core-features-and-limits`), `None` for a name wgpu cannot
/// give or the spec does not define.
fn wgpu_feature(name: &str) -> Option<wgpu::Features> {
    match name {
        "core-features-and-limits" => Some(wgpu::Features::empty()),
        "subgroups" => Some(wgpu::Features::SUBGROUP),
        "texture-formats-tier1" => Some(wgpu::Features::TEXTURE_FORMAT_16BIT_NORM),
        "immediates" => None,
        other => other
            .parse::<wgpu::Features>()
            .ok()
            .filter(|f| wgpu::Features::all_webgpu_mask().contains(*f)),
    }
}

/// The WebGPU names of `features`, sorted.
pub(crate) fn names(features: wgpu::Features) -> Vec<String> {
    let mut names: Vec<String> = SPEC_NAMES
        .iter()
        .filter(|name| wgpu_feature(name).is_some_and(|f| features.contains(f)))
        .map(|name| (*name).to_owned())
        .collect();
    names.sort();
    names
}

/// The features `names` name. A `TypeError` for a name WebGPU does not
/// define (WebIDL's enum check), and for a spec name wgpu cannot give (the
/// spec's `TypeError` for a feature the adapter lacks).
pub(crate) fn parse(names: &[String]) -> Result<wgpu::Features, NativeError> {
    let mut features = wgpu::Features::empty();
    for name in names {
        let feature = wgpu_feature(name).ok_or_else(|| {
            if SPEC_NAMES.contains(&name.as_str()) {
                NativeError::type_error(format!(
                    "requestDevice: the adapter does not support '{name}' (wgpu has no such feature)"
                ))
            } else {
                NativeError::type_error(format!("'{name}' is not a valid GPUFeatureName"))
            }
        })?;
        features |= feature;
    }
    Ok(features)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_webgpu_feature_of_wgpu_has_a_spec_name() {
        for (flag, feature) in wgpu::Features::all_webgpu_mask().iter_names() {
            if feature == wgpu::Features::IMMEDIATES {
                continue;
            }
            let reported = names(feature);
            assert_eq!(reported.len(), 2, "{flag}: {reported:?}");
            assert!(
                reported.iter().all(|n| SPEC_NAMES.contains(&n.as_str())),
                "{flag}"
            );
        }
    }

    #[test]
    fn names_round_trip_and_never_include_an_empty_name() {
        let all = wgpu::Features::all();
        let reported = names(all);
        assert!(reported.iter().all(|n| !n.is_empty()));
        assert!(reported.contains(&"core-features-and-limits".to_owned()));
        assert!(reported.contains(&"subgroups".to_owned()));
        assert!(!reported.contains(&"immediates".to_owned()));
        let parsed = parse(&reported).unwrap();
        assert_eq!(names(parsed), reported);
        assert!(names(wgpu::Features::empty()) == vec!["core-features-and-limits"]);
    }

    #[test]
    fn unknown_and_unavailable_names_are_type_errors() {
        for name in ["no-such-feature", "immediates", "texture-formats-tier2", ""] {
            let error = parse(&[name.to_owned()]).unwrap_err();
            assert!(
                error.to_string().starts_with("TypeError"),
                "{name}: {error}"
            );
        }
    }
}
