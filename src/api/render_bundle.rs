//! `GPURenderBundle`: what `bundleEncoder.finish()` makes
//! (`WgpuRenderBundleEncoder.finish`) and `pass.executeBundles` runs. No
//! functions of its own.

use crate::registry::Id;

/// A `GPURenderBundle` in the registry.
#[derive(Clone)]
pub struct RenderBundleEntry {
    /// The wgpu object; `None` for an invalid bundle (executing one makes
    /// the pass invalid).
    pub bundle: Option<wgpu::RenderBundle>,
    /// Its device.
    pub device: Id,
}

use nsh_core::prelude::*;

use crate::{error::guard, registry};

/// The native half of `GPURenderBundle`.
#[module]
pub struct WgpuRenderBundle;

#[module]
impl WgpuRenderBundle {
    /// Whether bundle `bundle` is valid (finished without a validation
    /// error).
    pub fn is_valid(bundle: u32) -> Result<bool, String> {
        guard(|| Ok(registry::get::<RenderBundleEntry>(bundle)?.bundle.is_some()))
    }
}
