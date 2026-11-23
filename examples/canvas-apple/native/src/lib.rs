//! The canvas example's own crate: nothing to export. It exists so the
//! app's hosts take nsh's crates from the Git revision nsh-webgpu uses.

use nsh_core::prelude::*;

#[native_module("canvas-apple/native")]
fn native(_registry: &mut Registry) {}
