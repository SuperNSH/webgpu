//! The canvas: a native view per platform (`webgpu.canvas`) whose layer or
//! window becomes a `wgpu::Surface`, and the offscreen fallback. See
//! docs/architecture.md "Canvas".
//!
//! The contract between the platform files and the canvas context
//! (`api/canvas_context.rs`) is [`views`]: a platform's view, when it is
//! made (on the UI thread), creates its surface from [`crate::instance`]
//! and [`views::attach`]es it under the context id TSX gave it as a prop;
//! it reports size changes with [`views::resize`] and goes with
//! [`crate::api::canvas_context::view_gone`], which ends the frame in
//! flight before it detaches and drops the surface. The context, on the JS
//! thread, configures and presents the attached surface.

pub mod entries;
pub mod views;

#[cfg(target_os = "android")]
pub mod android;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod apple;
#[cfg(target_os = "ios")]
pub mod ios;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod offscreen;
#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(windows)]
pub mod windows;
