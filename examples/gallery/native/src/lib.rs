//! The gallery's own crate: nothing to export. It exists so the app's hosts
//! take nsh's crates from the Git revision nsh-webgpu uses.

use nsh_core::prelude::*;

#[native_module("gallery/native")]
fn native(_registry: &mut Registry) {}
