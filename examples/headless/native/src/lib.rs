//! The headless app's own crate: nothing to export. It exists so the app's
//! hosts take nsh's crates from the Git revision nsh-webgpu uses (an app
//! with nshm only builds against crates.io's nsh).

use nsh_core::prelude::*;

#[native_module("headless/native")]
fn native(_registry: &mut Registry) {}
