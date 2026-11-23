//! iOS: the `webgpu.canvas` component, a UIView whose layer is a
//! CAMetalLayer (`+layerClass`, as the reference's `MetalView`) (area:
//! surfaces-apple).
//!
//! The view reports its size in points and its screen's scale when they
//! change (`layoutSubviews`, `didMoveToWindow`), and takes no touches
//! itself (`userInteractionEnabled` off), so they reach the container NSH
//! draws around it and the `<Canvas>`'s own handlers. Registered for every
//! view the iOS hosts make by the `#[native_components(ios)]` entry below.

use std::{cell::Cell, rc::Rc};

use nsh_core::{JsonValue, prelude::*};
use nsh_platform_ios::{ComponentContext, Components, NativeComponent};
use objc2::{
    ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::{Allocated, Retained},
    runtime::AnyClass,
};
use objc2_core_foundation::CGRect;
use objc2_foundation::{NSObject, NSObjectProtocol};
use objc2_quartz_core::CAMetalLayer;
use objc2_ui_kit::{UIResponder, UITraitEnvironment, UIView};

use super::apple::{self, COMPONENT, CanvasProps};

/// The view's state: the context it shows (0 before `contextId` came).
#[derive(Default)]
pub(crate) struct CanvasViewIvars {
    context_id: Cell<u32>,
}

define_class!(
    /// The canvas view: its layer is a CAMetalLayer.
    #[unsafe(super(UIView, UIResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "NshWebGPUCanvasView"]
    #[ivars = CanvasViewIvars]
    pub(crate) struct CanvasView;

    unsafe impl NSObjectProtocol for CanvasView {}

    impl CanvasView {
        #[unsafe(method(layerClass))]
        fn layer_class() -> &'static AnyClass {
            CAMetalLayer::class()
        }

        #[unsafe(method(layoutSubviews))]
        fn layout_subviews(&self) {
            // SAFETY: UIView's own.
            let _: () = unsafe { msg_send![super(self), layoutSubviews] };
            self.changed();
        }

        #[unsafe(method(didMoveToWindow))]
        fn did_move_to_window(&self) {
            // SAFETY: UIView's own.
            let _: () = unsafe { msg_send![super(self), didMoveToWindow] };
            self.changed();
        }
    }
);

impl CanvasView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this: Allocated<Self> = Self::alloc(mtm);
        let this = this.set_ivars(CanvasViewIvars::default());
        // SAFETY: UIView's designated initializer.
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: CGRect::ZERO] };
        view.setUserInteractionEnabled(false);
        view
    }

    /// Its layer.
    fn metal_layer(&self) -> Option<Retained<CAMetalLayer>> {
        self.layer().downcast::<CAMetalLayer>().ok()
    }

    /// Device pixels per point where the view is (its trait collection's
    /// display scale; 1 before it is in a window).
    fn scale(&self) -> f64 {
        // SAFETY: a plain getter of the view's trait collection.
        let scale = unsafe { self.traitCollection().displayScale() };
        if scale > 0.0 { scale } else { 1.0 }
    }

    /// Its size or scale changed: tell the context.
    fn changed(&self) {
        let id = self.ivars().context_id.get();
        if id == 0 {
            return;
        }
        let scale = self.scale();
        self.setContentScaleFactor(scale);
        if let Some(layer) = self.metal_layer() {
            let size = self.bounds().size;
            apple::resize(id, &layer, size.width, size.height, scale);
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
        let scale = self.scale();
        self.setContentScaleFactor(scale);
        let size = self.bounds().size;
        apple::attach(id, &layer, size.width, size.height, scale);
    }
}

/// The `webgpu.canvas` component: its view and props.
struct Canvas {
    view: Retained<CanvasView>,
    props: CanvasProps,
}

impl NativeComponent for Canvas {
    fn view(&self) -> &UIView {
        &self.view
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

/// Registers `webgpu.canvas` for the views the iOS hosts make.
#[native_components(ios)]
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
