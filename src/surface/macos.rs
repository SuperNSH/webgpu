//! macOS: the `webgpu.canvas` component, an NSView hosting a CAMetalLayer
//! (area: surfaces-apple).
//!
//! The view hosts the layer (`setLayer` then `setWantsLayer`, as the
//! reference's `MetalView` on macOS), reports its size in points and its
//! window's backing scale whenever either changes, and lets the pointer
//! through to the container NSH draws around it (`hitTest:` is nil), so the
//! `<Canvas>`'s own handlers get presses and drags, as a React Native view
//! does. Registered for every view the hosts make by the
//! `#[native_components(macos)]` entry below (re-exported at the crate's
//! root by `entries.rs`).

use std::{cell::Cell, rc::Rc};

use nsh_core::{JsonValue, prelude::*};
use nsh_platform_macos::{ComponentContext, Components, NativeComponent};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::{Allocated, Retained},
};
use objc2_app_kit::{NSResponder, NSScreen, NSView};
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};
use objc2_quartz_core::CAMetalLayer;

use super::apple::{self, COMPONENT, CanvasProps};

/// The view's state: the context it shows (0 before `contextId` came).
#[derive(Default)]
pub(crate) struct CanvasViewIvars {
    context_id: Cell<u32>,
}

define_class!(
    /// The canvas view: hosts a CAMetalLayer, tracks its size and scale.
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "NshWebGPUCanvasView"]
    #[ivars = CanvasViewIvars]
    pub(crate) struct CanvasView;

    unsafe impl NSObjectProtocol for CanvasView {}

    impl CanvasView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> Option<Retained<NSView>> {
            None
        }

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: NSView's own setFrameSize:.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            self.changed();
        }

        #[unsafe(method(viewDidChangeBackingProperties))]
        fn did_change_backing_properties(&self) {
            // SAFETY: NSView's own.
            let _: () = unsafe { msg_send![super(self), viewDidChangeBackingProperties] };
            self.changed();
        }

        #[unsafe(method(viewDidMoveToWindow))]
        fn did_move_to_window(&self) {
            // SAFETY: NSView's own.
            let _: () = unsafe { msg_send![super(self), viewDidMoveToWindow] };
            self.changed();
        }
    }
);

impl CanvasView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this: Allocated<Self> = Self::alloc(mtm);
        let this = this.set_ivars(CanvasViewIvars::default());
        // SAFETY: NSView's designated initializer.
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        let layer = CAMetalLayer::new();
        // Layer-hosting: the view shows the layer and draws nothing itself.
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        view
    }

    /// The hosted layer.
    fn metal_layer(&self) -> Option<Retained<CAMetalLayer>> {
        self.layer()?.downcast::<CAMetalLayer>().ok()
    }

    /// Device pixels per point where the view is.
    fn scale(&self) -> f64 {
        if let Some(window) = self.window() {
            return window.backingScaleFactor();
        }
        NSScreen::mainScreen(self.mtm()).map_or(1.0, |s| s.backingScaleFactor())
    }

    /// Its size or scale changed: tell the context.
    fn changed(&self) {
        let id = self.ivars().context_id.get();
        if id == 0 {
            return;
        }
        if let Some(layer) = self.metal_layer() {
            let size = self.frame().size;
            apple::resize(id, &layer, size.width, size.height, self.scale());
        }
    }

    /// Tags the layer again (props that came after the id).
    fn retag(&self, display_p3: bool, extended: bool) {
        if let Some(layer) = self.metal_layer() {
            apple::tag_colorspace(&layer, display_p3, extended);
        }
    }

    /// Shows context `id`: tags the layer, makes and attaches its surface.
    fn attach(&self, id: u32, display_p3: bool, extended: bool) {
        let Some(layer) = self.metal_layer() else {
            return;
        };
        apple::tag_colorspace(&layer, display_p3, extended);
        self.ivars().context_id.set(id);
        let size = self.frame().size;
        apple::attach(id, &layer, size.width, size.height, self.scale());
    }
}

/// The `webgpu.canvas` component: its view and props.
struct Canvas {
    view: Retained<CanvasView>,
    props: CanvasProps,
}

impl NativeComponent for Canvas {
    fn view(&self) -> Retained<NSView> {
        Retained::into_super(self.view.clone())
    }

    fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
        let first = self.props.set(name, value);
        match self.props.context_id {
            Some(id) if first => self
                .view
                .attach(id, self.props.display_p3, self.props.extended),
            // The colorspace props are the canvas's mount values (Canvas.tsx
            // sends no change), which may come after the id.
            Some(_) if name != "contextId" => {
                self.view.retag(self.props.display_p3, self.props.extended)
            }
            _ => {}
        }
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        if let Some(id) = self.props.context_id {
            self.view.ivars().context_id.set(0);
            apple::detach(id);
        }
    }
}

/// Registers `webgpu.canvas` for the views the macOS hosts make.
#[native_components(macos)]
fn components(components: &Components) {
    components.register(
        COMPONENT,
        Rc::new(|_context: ComponentContext| -> Box<dyn NativeComponent> {
            let mtm = MainThreadMarker::new().expect("components are made on the main thread");
            Box::new(Canvas {
                view: CanvasView::new(mtm),
                props: CanvasProps::default(),
            })
        }),
    );
}
