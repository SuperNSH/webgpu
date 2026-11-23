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

/// `GPUExtent3D`: `[width, height?, depthOrArrayLayers?]` or the dictionary.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum Extent3d {
    /// The sequence form.
    Seq(Vec<u32>),
    /// The dictionary form.
    Dict {
        /// Width.
        width: u32,
        /// Height (1).
        #[serde(default = "one")]
        height: u32,
        /// Depth or layers (1).
        #[serde(default = "one", rename = "depthOrArrayLayers")]
        depth_or_array_layers: u32,
    },
}

fn one() -> u32 {
    1
}

impl Extent3d {
    /// As wgpu's, or a `TypeError` for a sequence of 0 or more than 3.
    pub(crate) fn to_wgpu(&self) -> Result<wgpu::Extent3d, NativeError> {
        match self {
            Extent3d::Seq(v) if (1..=3).contains(&v.len()) => Ok(wgpu::Extent3d {
                width: v[0],
                height: v.get(1).copied().unwrap_or(1),
                depth_or_array_layers: v.get(2).copied().unwrap_or(1),
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
                width: *width,
                height: *height,
                depth_or_array_layers: *depth_or_array_layers,
            }),
        }
    }
}

/// `GPUOrigin3D`: `[x?, y?, z?]` or the dictionary (each 0 by default).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum Origin3d {
    /// The sequence form.
    Seq(Vec<u32>),
    /// The dictionary form.
    Dict {
        /// x.
        #[serde(default)]
        x: u32,
        /// y.
        #[serde(default)]
        y: u32,
        /// z.
        #[serde(default)]
        z: u32,
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
                x: v.first().copied().unwrap_or(0),
                y: v.get(1).copied().unwrap_or(0),
                z: v.get(2).copied().unwrap_or(0),
            }),
            Origin3d::Seq(v) => Err(NativeError::type_error(format!(
                "a GPUOrigin3D sequence has at most 3 members, not {}",
                v.len()
            ))),
            Origin3d::Dict { x, y, z } => Ok(wgpu::Origin3d {
                x: *x,
                y: *y,
                z: *z,
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
