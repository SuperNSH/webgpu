//! What macOS and iOS share: the `webgpu.canvas` component's props, the
//! CAMetalLayer's colorspace, and the wgpu surface made from the layer
//! (area: surfaces-apple). `macos.rs` and `ios.rs` make the view (an
//! NSView hosting the layer, a UIView whose layer it is) and call these on
//! the main thread.
//!
//! As react-native-webgpu's `MetalView` (`apple/MetalView.mm`): the layer
//! is tagged with its colorspace on the main thread when the canvas mounts,
//! then its surface is made with
//! `SurfaceTargetUnsafe::CoreAnimationLayer` (wgpu retains the layer and
//! uses it from the JS thread, which `CAMetalLayer` allows) and attached
//! under the context id TSX gave the view.

use objc2_core_foundation::CGSize;
use objc2_core_graphics::{
    CGColorSpace, kCGColorSpaceExtendedDisplayP3, kCGColorSpaceExtendedSRGB,
};
use objc2_quartz_core::CAMetalLayer;
use serde_json::Value as JsonValue;

use crate::{api::canvas_context, surface::views};

/// The native component's name: `<Canvas>` renders
/// `nativeComponent("webgpu.canvas")`.
pub const COMPONENT: &str = "webgpu.canvas";

/// The canvas view's props, as `Canvas.tsx` sends them.
#[derive(Debug, Default)]
pub(crate) struct CanvasProps {
    /// `contextId`: set once, when the canvas mounts.
    pub(crate) context_id: Option<u32>,
    /// `appleDisplayP3`: the `colorSpace` prop is `"display-p3"`.
    pub(crate) display_p3: bool,
    /// `appleExtendedDynamicRange`: `toneMapping.mode` is `"extended"`.
    pub(crate) extended: bool,
}

impl CanvasProps {
    /// Takes prop `name`; whether it was the context id, arriving for the
    /// first time (the view attaches then).
    pub(crate) fn set(&mut self, name: &str, value: Option<&JsonValue>) -> bool {
        match name {
            "contextId" => {
                let id = value
                    .and_then(JsonValue::as_u64)
                    .and_then(|v| u32::try_from(v).ok());
                if self.context_id.is_none() && id.is_some() {
                    self.context_id = id;
                    return true;
                }
                false
            }
            "appleDisplayP3" => {
                self.display_p3 = value.and_then(JsonValue::as_bool).unwrap_or(false);
                false
            }
            "appleExtendedDynamicRange" => {
                self.extended = value.and_then(JsonValue::as_bool).unwrap_or(false);
                false
            }
            _ => false,
        }
    }
}

/// Tags `layer` with the colorspace of the canvas's values, as the
/// reference's `applyCAMetalLayerColorSpace`: extended sRGB, or extended
/// Display P3, gamma encoded, so a float canvas shows the same colors as an
/// 8-bit one; extended dynamic range for `toneMapping: "extended"`. Main
/// thread only.
pub(crate) fn tag_colorspace(layer: &CAMetalLayer, display_p3: bool, extended: bool) {
    // SAFETY: the extern statics are CoreGraphics' constant names.
    let name = unsafe {
        if display_p3 {
            kCGColorSpaceExtendedDisplayP3
        } else {
            kCGColorSpaceExtendedSRGB
        }
    };
    let space = CGColorSpace::with_name(Some(name));
    layer.setColorspace(space.as_deref());
    layer.setWantsExtendedDynamicRangeContent(extended);
}

/// The view of `context_id` shows `layer`, `width` × `height` points at
/// `scale`: makes its surface and attaches it. Main thread.
pub(crate) fn attach(context_id: u32, layer: &CAMetalLayer, width: f64, height: f64, scale: f64) {
    layer.setContentsScale(scale);
    let pointer = std::ptr::from_ref(layer) as *mut core::ffi::c_void;
    // SAFETY: `layer` is a live CAMetalLayer; wgpu retains it for as long as
    // the surface lives.
    let surface = unsafe {
        crate::instance::instance()
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(pointer))
    };
    let surface = match surface {
        Ok(surface) => surface,
        Err(error) => {
            eprintln!("nsh-webgpu: canvas {context_id}: no surface for its layer: {error}");
            return;
        }
    };
    let (w, h) = pixels(width, height, scale);
    views::attach(context_id, surface, w, h);
    canvas_context::set_native(context_id, pointer as usize, width, height, scale);
    canvas_context::surface_attached(context_id);
}

/// The view of `context_id` is now `width` × `height` points at `scale`.
/// Main thread.
pub(crate) fn resize(context_id: u32, layer: &CAMetalLayer, width: f64, height: f64, scale: f64) {
    layer.setContentsScale(scale);
    let (w, h) = pixels(width, height, scale);
    views::resize(context_id, w, h);
    let pointer = std::ptr::from_ref(layer) as usize;
    canvas_context::set_native(context_id, pointer, width, height, scale);
}

/// The view of `context_id` went: its frame in flight ends, then its
/// surface goes ([`canvas_context::view_gone`]). Main thread; never panics.
pub(crate) fn detach(context_id: u32) {
    canvas_context::view_gone(context_id);
}

/// `width` × `height` points at `scale`, in whole pixels.
fn pixels(width: f64, height: f64, scale: f64) -> (u32, u32) {
    let size = CGSize::new(width * scale, height * scale);
    (
        size.width.round().max(0.0) as u32,
        size.height.round().max(0.0) as u32,
    )
}
