//! Linux: the canvas is a `gtk::Picture` showing frames rendered offscreen
//! ([`super::offscreen`]).
//!
//! A `wgpu::Surface` needs a native window or Wayland surface of its own,
//! and a GTK 4 widget has neither: GTK 4 dropped child windows, and only a
//! toplevel's `GdkSurface` maps to an X11 window or `wl_surface`, which GTK
//! itself renders the whole window into (with GL or Vulkan), so a swapchain
//! there would fight GTK's renderer. Wayland subsurfaces are GTK's own
//! business (`GtkGraphicsOffload` takes dmabuf textures, not a surface we
//! could hand wgpu). So the canvas context renders into a texture, and
//! `present()` reads it back off the JS thread and hands the pixels to the
//! picture as a `gdk::MemoryTexture`, on the GTK main loop.
//!
//! Cost: one GPU→CPU copy and one CPU→GPU upload per presented frame (a
//! 1080p frame is 8 MB each way); a frame presented while the last one is
//! still being read back is dropped. A dmabuf export (Vulkan external
//! memory into `GdkDmabufTextureBuilder`) would make it zero-copy; wgpu 29
//! has no API for exporting a texture's memory.
//!
//! The view's props: `contextId`. Its size in pixels (allocation × scale
//! factor) follows the widget on GTK's frame clock and goes to
//! [`super::offscreen::resize`], so the next frame's texture has it.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

use nsh_core::{JsonValue, prelude::*};
use nsh_platform_linux::{
    ComponentContext, Components, NativeComponent,
    gtk4::{self as gtk, gdk, glib, prelude::*},
};

use super::offscreen::{self, Frame};
use crate::api::canvas_context;

/// The name `<Canvas>` makes its view with (`nativeComponent("webgpu.canvas")`).
pub const COMPONENT: &str = "webgpu.canvas";

thread_local! {
    /// The pictures on screen, by context id (GTK main thread only): where
    /// a presented frame, posted from the readback thread, goes.
    static PICTURES: RefCell<BTreeMap<u32, gtk::Picture>> = const { RefCell::new(BTreeMap::new()) };
}

#[native_components(linux)]
fn components(components: &Components) {
    components.register(
        COMPONENT,
        Rc::new(|context: ComponentContext| -> Box<dyn NativeComponent> {
            Box::new(Canvas::new(context))
        }),
    );
}

/// The canvas view.
struct Canvas {
    picture: gtk::Picture,
    context_id: Rc<RefCell<u32>>,
    tick: Option<gtk::TickCallbackId>,
    _context: ComponentContext,
}

impl Canvas {
    fn new(context: ComponentContext) -> Self {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Fill);
        picture.set_can_shrink(true);
        picture.set_hexpand(true);
        picture.set_vexpand(true);
        let context_id = Rc::new(RefCell::new(0_u32));
        // The widget's size in pixels, checked on each frame of GTK's clock
        // (GTK 4 has no size signal on a plain widget).
        let id = Rc::clone(&context_id);
        let last = RefCell::new((0_u32, 0_u32));
        let tick = picture.add_tick_callback(move |widget, _clock| {
            let size = pixel_size(widget.upcast_ref());
            let id = *id.borrow();
            if id != 0 && *last.borrow() != size {
                *last.borrow_mut() = size;
                offscreen::resize(id, size.0, size.1);
                report(id, widget.upcast_ref());
            }
            glib::ControlFlow::Continue
        });
        Self {
            picture,
            context_id,
            tick: Some(tick),
            _context: context,
        }
    }

    fn show(&self, id: u32) {
        PICTURES.with(|p| p.borrow_mut().insert(id, self.picture.clone()));
        let (width, height) = pixel_size(self.picture.upcast_ref());
        offscreen::show(id, Arc::new(move |frame| post(id, frame)), width, height);
        report(id, self.picture.upcast_ref());
    }

    fn hide(id: u32) {
        offscreen::hide(id);
        canvas_context::view_gone(id);
        PICTURES.with(|p| p.borrow_mut().remove(&id));
    }
}

impl NativeComponent for Canvas {
    fn widget(&self) -> gtk::Widget {
        self.picture.clone().upcast()
    }

    fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
        if name != "contextId" {
            return;
        }
        let id = value
            .and_then(JsonValue::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0);
        let old = self.context_id.replace(id);
        if old == id {
            return;
        }
        if old != 0 {
            Self::hide(old);
        }
        if id != 0 {
            self.show(id);
        }
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        if let Some(tick) = self.tick.take() {
            tick.remove();
        }
        let id = self.context_id.replace(0);
        if id != 0 {
            Self::hide(id);
        }
    }
}

/// Tells the canvas context the view's size in points and its scale
/// (`getNativeSurface()`; no pointer: a picture has no native surface).
fn report(id: u32, widget: &gtk::Widget) {
    canvas_context::set_native(
        id,
        0,
        f64::from(widget.width()),
        f64::from(widget.height()),
        f64::from(widget.scale_factor().max(1)),
    );
}

/// The widget's size in device pixels (at least 1 × 1).
fn pixel_size(widget: &gtk::Widget) -> (u32, u32) {
    let scale = widget.scale_factor().max(1);
    (
        u32::try_from(widget.width() * scale).unwrap_or(0).max(1),
        u32::try_from(widget.height() * scale).unwrap_or(0).max(1),
    )
}

/// Hands `frame` (from the readback thread) to context `id`'s picture on
/// the GTK main loop.
fn post(id: u32, frame: Frame) {
    glib::MainContext::default().invoke(move || {
        let Some(picture) = PICTURES.with(|p| p.borrow().get(&id).cloned()) else {
            return;
        };
        if let Some(texture) = texture(frame) {
            picture.set_paintable(Some(&texture));
        }
    });
}

/// The frame as a texture GTK can draw; `None` for formats GDK has no
/// memory format for.
fn texture(frame: Frame) -> Option<gdk::MemoryTexture> {
    use wgpu::TextureFormat as F;
    // An opaque canvas's alpha is ignored, as `alphaMode: "opaque"` says.
    let (format, bpp) = match frame.format {
        F::Rgba8Unorm | F::Rgba8UnormSrgb if frame.opaque => (gdk::MemoryFormat::R8g8b8x8, 4),
        F::Bgra8Unorm | F::Bgra8UnormSrgb if frame.opaque => (gdk::MemoryFormat::B8g8r8x8, 4),
        F::Rgba8Unorm | F::Rgba8UnormSrgb => (gdk::MemoryFormat::R8g8b8a8Premultiplied, 4),
        F::Bgra8Unorm | F::Bgra8UnormSrgb => (gdk::MemoryFormat::B8g8r8a8Premultiplied, 4),
        F::Rgba16Float => (gdk::MemoryFormat::R16g16b16a16FloatPremultiplied, 8),
        F::Rgba32Float => (gdk::MemoryFormat::R32g32b32a32FloatPremultiplied, 16),
        other => {
            log::warn!("webgpu.canvas: cannot show a {other:?} canvas on Linux");
            return None;
        }
    };
    let stride = frame.width as usize * bpp;
    let bytes = glib::Bytes::from_owned(frame.data);
    Some(gdk::MemoryTexture::new(
        frame.width as i32,
        frame.height as i32,
        format,
        &bytes,
        stride,
    ))
}
