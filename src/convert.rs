//! Descriptors from TSX.
//!
//! A descriptor crosses as a plain object (`JsonValue`): TSX's `wire()`
//! (`tsx/internal/wire.ts`) replaces each GPU object in it with its id, so
//! `{ module: shaderModule }` arrives as `{ "module": 7 }`. Each kind's file
//! declares the serde shape of its descriptor with the spec's member names
//! and defaults, and wgpu's own enums deserialize the spec's strings
//! (`"triangle-list"`, `"rgba8unorm"`: wgpu's `serde` feature). A shape that
//! does not convert is a `TypeError`, as WebIDL conversion is.
//!
//! This file has the dictionaries several kinds share.

use serde::{Deserialize, de::DeserializeOwned};

use crate::{error::NativeError, registry::Id};

/// `value` as a `T`, or a `TypeError` naming `what` and the member.
pub(crate) fn parse<T: DeserializeOwned>(
    what: &str,
    value: nsh_core::JsonValue,
) -> Result<T, NativeError> {
    serde_path_to_string(value).map_err(|e| NativeError::type_error(format!("{what}: {e}")))
}

fn serde_path_to_string<T: DeserializeOwned>(
    value: nsh_core::JsonValue,
) -> Result<T, serde_json::Error> {
    // `undefined` arrives as null: a missing optional dictionary.
    let value = if value.is_null() {
        nsh_core::JsonValue::Object(Default::default())
    } else {
        value
    };
    serde_json::from_value(value)
}

/// A `GPUIntegerCoordinate` (`[EnforceRange] unsigned long`): a fraction is
/// truncated (a canvas of 411.43 points at 2.625 pixels each is 1079.99…
/// wide), what is not a finite number of that range is a `TypeError`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Coordinate(u32);

impl<'de> Deserialize<'de> for Coordinate {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let n = f64::deserialize(deserializer)?.trunc();
        if (0.0..=f64::from(u32::MAX)).contains(&n) {
            Ok(Self(n as u32))
        } else {
            Err(serde::de::Error::custom(
                "outside the range of unsigned long",
            ))
        }
    }
}

/// `GPUExtent3D`: `[width, height?, depthOrArrayLayers?]` or the dictionary.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum Extent3d {
    /// The sequence form.
    Seq(Vec<Coordinate>),
    /// The dictionary form.
    Dict {
        /// Width.
        width: Coordinate,
        /// Height (1).
        #[serde(default = "one")]
        height: Coordinate,
        /// Depth or layers (1).
        #[serde(default = "one", rename = "depthOrArrayLayers")]
        depth_or_array_layers: Coordinate,
    },
}

fn one() -> Coordinate {
    Coordinate(1)
}

impl Extent3d {
    /// As wgpu's, or a `TypeError` for a sequence of 0 or more than 3.
    pub(crate) fn to_wgpu(&self) -> Result<wgpu::Extent3d, NativeError> {
        match self {
            Extent3d::Seq(v) if (1..=3).contains(&v.len()) => Ok(wgpu::Extent3d {
                width: v[0].0,
                height: v.get(1).map_or(1, |c| c.0),
                depth_or_array_layers: v.get(2).map_or(1, |c| c.0),
            }),
            Extent3d::Seq(v) => Err(NativeError::type_error(format!(
                "a GPUExtent3D sequence has 1 to 3 members, not {}",
                v.len()
            ))),
            Extent3d::Dict {
                width,
                height,
                depth_or_array_layers,
            } => Ok(wgpu::Extent3d {
                width: width.0,
                height: height.0,
                depth_or_array_layers: depth_or_array_layers.0,
            }),
        }
    }
}

/// `GPUOrigin3D`: `[x?, y?, z?]` or the dictionary (each 0 by default).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum Origin3d {
    /// The sequence form.
    Seq(Vec<Coordinate>),
    /// The dictionary form.
    Dict {
        /// x.
        #[serde(default)]
        x: Coordinate,
        /// y.
        #[serde(default)]
        y: Coordinate,
        /// z.
        #[serde(default)]
        z: Coordinate,
    },
}

impl Default for Origin3d {
    fn default() -> Self {
        Origin3d::Seq(Vec::new())
    }
}

impl Origin3d {
    /// As wgpu's, or a `TypeError` for a sequence of more than 3.
    pub(crate) fn to_wgpu(&self) -> Result<wgpu::Origin3d, NativeError> {
        match self {
            Origin3d::Seq(v) if v.len() <= 3 => Ok(wgpu::Origin3d {
                x: v.first().map_or(0, |c| c.0),
                y: v.get(1).map_or(0, |c| c.0),
                z: v.get(2).map_or(0, |c| c.0),
            }),
            Origin3d::Seq(v) => Err(NativeError::type_error(format!(
                "a GPUOrigin3D sequence has at most 3 members, not {}",
                v.len()
            ))),
            Origin3d::Dict { x, y, z } => Ok(wgpu::Origin3d {
                x: x.0,
                y: y.0,
                z: z.0,
            }),
        }
    }
}

/// `GPUColor`: `[r, g, b, a]` or `{ r, g, b, a }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum Color {
    /// The sequence form (exactly 4).
    Seq(Vec<f64>),
    /// The dictionary form.
    Dict {
        /// Red.
        r: f64,
        /// Green.
        g: f64,
        /// Blue.
        b: f64,
        /// Alpha.
        a: f64,
    },
}

impl Color {
    /// As wgpu's, or a `TypeError` for a sequence other than 4 long.
    pub(crate) fn to_wgpu(&self) -> Result<wgpu::Color, NativeError> {
        match self {
            Color::Seq(v) if v.len() == 4 => Ok(wgpu::Color {
                r: v[0],
                g: v[1],
                b: v[2],
                a: v[3],
            }),
            Color::Seq(v) => Err(NativeError::type_error(format!(
                "a GPUColor sequence has 4 members, not {}",
                v.len()
            ))),
            Color::Dict { r, g, b, a } => Ok(wgpu::Color {
                r: *r,
                g: *g,
                b: *b,
                a: *a,
            }),
        }
    }
}

/// `GPUTexelCopyBufferLayout` (and the buffer part of
/// `GPUTexelCopyBufferInfo`, whose `buffer` the caller reads).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TexelCopyBufferLayout {
    /// Offset into the buffer or data.
    #[serde(default)]
    pub(crate) offset: u64,
    /// Bytes per row.
    pub(crate) bytes_per_row: Option<u32>,
    /// Rows per image.
    pub(crate) rows_per_image: Option<u32>,
}

impl TexelCopyBufferLayout {
    /// As wgpu's.
    pub(crate) fn to_wgpu(&self) -> wgpu::TexelCopyBufferLayout {
        wgpu::TexelCopyBufferLayout {
            offset: self.offset,
            bytes_per_row: self.bytes_per_row,
            rows_per_image: self.rows_per_image,
        }
    }
}

/// `GPUTexelCopyBufferInfo`: a buffer id and its layout.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TexelCopyBufferInfo {
    /// The buffer's id.
    pub(crate) buffer: Id,
    /// The layout.
    #[serde(flatten)]
    pub(crate) layout: TexelCopyBufferLayout,
}

/// `GPUTexelCopyTextureInfo`: a texture id, mip level, origin and aspect.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TexelCopyTextureInfo {
    /// The texture's id.
    pub(crate) texture: Id,
    /// Mip level (0).
    #[serde(default)]
    pub(crate) mip_level: u32,
    /// Origin (0, 0, 0).
    #[serde(default)]
    pub(crate) origin: Origin3d,
    /// Aspect (`"all"`).
    #[serde(default)]
    pub(crate) aspect: wgpu::TextureAspect,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Extent3d, Origin3d, parse};

    /// WebIDL's `[EnforceRange]`: a fraction is truncated.
    #[test]
    fn coordinates_are_truncated() {
        let extent = |v| parse::<Extent3d>("GPUExtent3D", v).and_then(|e| e.to_wgpu());
        let size = extent(json!([1079.9999, 2029.5])).expect("a size");
        assert_eq!((size.width, size.height), (1079, 2029));
        let size = extent(json!({ "width": 3.7, "depthOrArrayLayers": 2.0 })).expect("a size");
        assert_eq!((size.width, size.height), (3, 1));
        assert_eq!(size.depth_or_array_layers, 2);
        assert!(extent(json!([-1])).is_err());
        assert!(extent(json!([4_294_967_296_u64])).is_err());
        let origin = parse::<Origin3d>("GPUOrigin3D", json!([0.5, 2.9])).and_then(|o| o.to_wgpu());
        let origin = origin.expect("an origin");
        assert_eq!((origin.x, origin.y, origin.z), (0, 2, 0));
    }
}
