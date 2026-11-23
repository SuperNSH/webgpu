//! Android: the canvas is a `FrameLayout` holding a `TextureView` (the
//! default) or a `SurfaceView`, as react-native-webgpu's `WebGPUView`
//! does; its `Surface`'s `ANativeWindow` becomes the `wgpu::Surface`
//! (`SurfaceTargetUnsafe::RawHandle`, Android NDK), for Vulkan or GLES.
//!
//! An nshm ships no Java (the CLI builds only NSH's own classes), so the
//! view cannot implement `SurfaceHolder.Callback` or
//! `TextureView.SurfaceTextureListener`. Instead it follows its surface
//! on the UI thread's `AChoreographer` (an NDK C callback each vsync while
//! the component lives): when the surface becomes valid it makes and
//! attaches the `wgpu::Surface` ([`super::views::attach`]), when the view's
//! size changes it reports it ([`super::views::resize`]), when the surface
//! goes it detaches it. Each check is a few JNI calls per frame. The
//! difference from the callbacks: a surface destroyed by the system is
//! detached on the next vsync rather than before `surfaceDestroyed`
//! returns, so a present racing it fails with a surface error (the
//! `ANativeWindow` stays referenced by the swapchain; nothing is freed
//! under it).
//!
//! Props, the reference's resolved `android` props (`Canvas.tsx`
//! `androidNativeProps`): `contextId`, `androidBackingView`
//! (`"TextureView"` default, `"SurfaceView"`; `"HardwareBufferView"` falls
//! back to a TextureView with a warning: it needs the reference's Java
//! `HardwareRenderer` path), `androidOpaque`, `androidZOrderOnTop`,
//! `androidZOrderMediaOverlay` (SurfaceView only). The backing view is
//! made on the first vsync after the props arrive and is fixed from then
//! on, as in the reference (a later change is logged and ignored); opacity
//! and z-order apply live.

use std::{cell::RefCell, ffi::c_void, ptr::NonNull, rc::Rc};

use nsh::{
    ComponentContext, Components, NativeComponent,
    jni::{
        JNIEnv,
        objects::{GlobalRef, JObject, JValue},
        sys::jobject,
    },
};
use nsh_core::{JsonValue, prelude::*};

use super::views;
use crate::api::canvas_context;

/// The name `<Canvas>` makes its view with (`nativeComponent("webgpu.canvas")`).
pub const COMPONENT: &str = "webgpu.canvas";

/// `android.graphics.PixelFormat.OPAQUE` / `TRANSLUCENT`.
const PIXEL_FORMAT_OPAQUE: i32 = -1;
const PIXEL_FORMAT_TRANSLUCENT: i32 = -3;
/// `ViewGroup.LayoutParams.MATCH_PARENT`.
const MATCH_PARENT: i32 = -1;
/// `Build.VERSION_CODES.Q`: TextureView's opaque flag misrenders there
/// (the reference's `WebGPUTextureView.setOpaque`).
const ANDROID_Q: i32 = 29;

#[allow(non_camel_case_types)]
type AChoreographer_frameCallback = unsafe extern "C" fn(frame_time_nanos: i64, data: *mut c_void);

#[link(name = "android")]
unsafe extern "C" {
    fn ANativeWindow_fromSurface(env: *mut nsh::jni::sys::JNIEnv, surface: jobject) -> *mut c_void;
    fn ANativeWindow_release(window: *mut c_void);
    fn AChoreographer_getInstance() -> *mut c_void;
    fn AChoreographer_postFrameCallback(
        choreographer: *mut c_void,
        callback: AChoreographer_frameCallback,
        data: *mut c_void,
    );
}

#[native_components(android)]
fn components(components: &Components) {
    components.register(
        COMPONENT,
        Rc::new(|context: ComponentContext| -> Box<dyn NativeComponent> {
            // A panic here is caught by NSH, which shows an empty view.
            Box::new(
                Canvas::new(context)
                    .unwrap_or_else(|err| panic!("webgpu.canvas: cannot make the view: {err}")),
            )
        }),
    );
}

/// The backing view kinds (`AndroidBackingView`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backing {
    TextureView,
    SurfaceView,
}

/// What the props asked for, and what the view has made of them. UI
/// thread only (the component and its vsync callback).
struct State {
    alive: bool,
    android_context: GlobalRef,
    container: GlobalRef,
    context_id: u32,
    /// The requested kind (`androidBackingView`), once the props came.
    requested: Option<Backing>,
    opaque: Option<bool>,
    z_on_top: bool,
    z_media_overlay: bool,
    /// The backing view, once made.
    child: Option<(Backing, GlobalRef)>,
    /// TextureView: the `SurfaceTexture` the current `Surface` was made
    /// from, and that `Surface` (ours to release).
    texture: Option<(GlobalRef, GlobalRef)>,
    /// The window the attached surface was made from (one reference ours).
    window: Option<NonNull<c_void>>,
    /// The context id and size attached under.
    attached: Option<(u32, u32, u32)>,
}

impl State {
    /// Detaches the surface and lets the window and our `Surface` go.
    fn detach(&mut self, env: &mut JNIEnv<'_>) {
        if let Some((id, _, _)) = self.attached.take() {
            canvas_context::view_gone(id);
        }
        if let Some(window) = self.window.take() {
            // SAFETY: the reference `ANativeWindow_fromSurface` gave us; the
            // swapchain made from it holds its own.
            unsafe { ANativeWindow_release(window.as_ptr()) };
        }
        if let Some((_, surface)) = self.texture.take() {
            let _ = env.call_method(surface.as_obj(), "release", "()V", &[]);
        }
    }
}

/// The canvas view.
struct Canvas {
    container: GlobalRef,
    state: Rc<RefCell<State>>,
}

impl Canvas {
    fn new(context: ComponentContext) -> nsh::jni::errors::Result<Self> {
        let android_context = context.android_context().clone();
        let container = context.with_env(|env| {
            let layout = env.new_object(
                "android/widget/FrameLayout",
                "(Landroid/content/Context;)V",
                &[JValue::Object(android_context.as_obj())],
            )?;
            env.new_global_ref(layout)
        })?;
        let state = Rc::new(RefCell::new(State {
            alive: true,
            android_context,
            container: container.clone(),
            context_id: 0,
            requested: None,
            opaque: None,
            z_on_top: false,
            z_media_overlay: false,
            child: None,
            texture: None,
            window: None,
            attached: None,
        }));
        // SAFETY: on the UI thread, which has the looper the choreographer
        // needs; the callback's data is an `Rc` it owns until `alive` is
        // false.
        unsafe {
            let choreographer = AChoreographer_getInstance();
            if !choreographer.is_null() {
                AChoreographer_postFrameCallback(
                    choreographer,
                    on_frame,
                    Rc::into_raw(Rc::clone(&state)) as *mut c_void,
                );
            } else {
                log::error!("webgpu.canvas: no choreographer on this thread");
            }
        }
        Ok(Self { container, state })
    }
}

impl NativeComponent for Canvas {
    fn view(&self) -> &GlobalRef {
        &self.container
    }

    fn set_prop(&mut self, env: &mut JNIEnv<'_>, name: &str, value: Option<&JsonValue>) {
        let mut state = self.state.borrow_mut();
        let flag = || value.and_then(JsonValue::as_bool);
        match name {
            "contextId" => {
                let id = value
                    .and_then(JsonValue::as_u64)
                    .and_then(|v| u32::try_from(v).ok())
                    .unwrap_or(0);
                if id != state.context_id {
                    state.context_id = id;
                    // Re-attached under the new id on the next vsync.
                    state.detach(env);
                }
            }
            "androidBackingView" => {
                let kind = match value.and_then(JsonValue::as_str) {
                    Some("SurfaceView") => Backing::SurfaceView,
                    Some("HardwareBufferView") => {
                        log::warn!(
                            "webgpu.canvas: backingView \"HardwareBufferView\" is not supported \
                             by nsh-webgpu; using a TextureView"
                        );
                        Backing::TextureView
                    }
                    _ => Backing::TextureView,
                };
                if let Some((made, _)) = &state.child
                    && *made != kind
                {
                    log::warn!(
                        "webgpu.canvas: ignoring a change of android.backingView on a mounted \
                         canvas: the backing view is chosen when the canvas mounts"
                    );
                }
                state.requested = Some(kind);
            }
            "androidOpaque" => {
                state.opaque = flag();
                apply_opaque(env, &state);
            }
            "androidZOrderOnTop" => {
                state.z_on_top = flag().unwrap_or(false);
                apply_z_order(env, &state);
            }
            "androidZOrderMediaOverlay" => {
                state.z_media_overlay = flag().unwrap_or(false);
                apply_z_order(env, &state);
            }
            _ => {}
        }
    }

    fn destroy(&mut self, env: &mut JNIEnv<'_>) {
        let mut state = self.state.borrow_mut();
        state.alive = false;
        state.detach(env);
    }
}

/// Whether the canvas composites as opaque (the reference's
/// `resolveOpaque`, for a prop that did not come).
fn opaque(state: &State, kind: Backing) -> bool {
    state.opaque.unwrap_or(kind == Backing::SurfaceView)
}

fn apply_opaque(env: &mut JNIEnv<'_>, state: &State) {
    let Some((kind, view)) = &state.child else {
        return;
    };
    let opaque = opaque(state, *kind);
    let _ = match kind {
        Backing::TextureView => {
            let sdk = sdk_int(env);
            env.call_method(
                view.as_obj(),
                "setOpaque",
                "(Z)V",
                &[JValue::Bool(u8::from(opaque && sdk != ANDROID_Q))],
            )
            .map(drop)
        }
        Backing::SurfaceView => holder(env, view).and_then(|holder| {
            env.call_method(
                &holder,
                "setFormat",
                "(I)V",
                &[JValue::Int(if opaque {
                    PIXEL_FORMAT_OPAQUE
                } else {
                    PIXEL_FORMAT_TRANSLUCENT
                })],
            )
            .map(drop)
        }),
    };
    clear_exception(env);
}

fn apply_z_order(env: &mut JNIEnv<'_>, state: &State) {
    let Some((Backing::SurfaceView, view)) = &state.child else {
        return;
    };
    let _ = env.call_method(
        view.as_obj(),
        "setZOrderOnTop",
        "(Z)V",
        &[JValue::Bool(u8::from(state.z_on_top))],
    );
    if !state.z_on_top {
        let _ = env.call_method(
            view.as_obj(),
            "setZOrderMediaOverlay",
            "(Z)V",
            &[JValue::Bool(u8::from(state.z_media_overlay))],
        );
    }
    let _ = env.call_method(view.as_obj(), "invalidate", "()V", &[]);
    clear_exception(env);
}

fn sdk_int(env: &mut JNIEnv<'_>) -> i32 {
    env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")
        .and_then(|v| v.i())
        .unwrap_or(0)
}

fn holder<'a>(env: &mut JNIEnv<'a>, view: &GlobalRef) -> nsh::jni::errors::Result<JObject<'a>> {
    env.call_method(
        view.as_obj(),
        "getHolder",
        "()Landroid/view/SurfaceHolder;",
        &[],
    )?
    .l()
}

fn clear_exception(env: &mut JNIEnv<'_>) {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
}

/// The vsync callback: follows the surface, then asks for the next vsync
/// while the component lives.
unsafe extern "C" fn on_frame(_frame_time_nanos: i64, data: *mut c_void) {
    // SAFETY: `data` is the `Rc` `Canvas::new` (or the last frame) gave up.
    let state = unsafe { Rc::from_raw(data as *const RefCell<State>) };
    if !state.borrow().alive {
        // Dropping the last `Rc` the callback held.
        return;
    }
    let result = nsh::with_env(|env| {
        follow(env, &mut state.borrow_mut());
        Ok(())
    });
    if let Err(err) = result {
        log::warn!("webgpu.canvas: {err}");
    }
    // SAFETY: still on the UI thread; the `Rc` goes back to the callback.
    unsafe {
        let choreographer = AChoreographer_getInstance();
        if choreographer.is_null() {
            return;
        }
        AChoreographer_postFrameCallback(
            choreographer,
            on_frame,
            Rc::into_raw(state) as *mut c_void,
        );
    }
}

/// One vsync's check: makes the backing view once the props came, then
/// attaches, resizes or detaches the surface as the view's has changed.
fn follow(env: &mut JNIEnv<'_>, state: &mut State) {
    if state.context_id == 0 {
        return;
    }
    if state.child.is_none() {
        let kind = state.requested.unwrap_or(Backing::TextureView);
        match make_child(env, state, kind) {
            Ok(view) => {
                state.child = Some((kind, view));
                apply_opaque(env, state);
                apply_z_order(env, state);
            }
            Err(err) => {
                clear_exception(env);
                log::error!("webgpu.canvas: cannot make the {kind:?}: {err}");
                state.alive = false;
                return;
            }
        }
    }
    let Some((kind, view)) = state.child.clone() else {
        return;
    };
    let surface = match current_surface(env, state, kind, &view) {
        Ok(surface) => surface,
        Err(err) => {
            clear_exception(env);
            log::warn!("webgpu.canvas: {err}");
            None
        }
    };
    let Some(surface) = surface else {
        if state.attached.is_some() {
            state.detach(env);
        }
        return;
    };
    let width = env
        .call_method(view.as_obj(), "getWidth", "()I", &[])
        .and_then(|v| v.i())
        .unwrap_or(0)
        .max(1) as u32;
    let height = env
        .call_method(view.as_obj(), "getHeight", "()I", &[])
        .and_then(|v| v.i())
        .unwrap_or(0)
        .max(1) as u32;
    match state.attached {
        Some((id, w, h)) if id == state.context_id => {
            if (w, h) != (width, height) {
                views::resize(id, width, height);
                state.attached = Some((id, width, height));
                report_native(env, state, &view, width, height);
            }
        }
        _ => {
            attach(env, state, surface.as_obj(), width, height);
            if let Some((id, _, _)) = state.attached
                && id == state.context_id
            {
                report_native(env, state, &view, width, height);
                canvas_context::surface_attached(id);
            }
        }
    }
}

/// Tells the canvas context the window's pointer, the view's size in dp
/// and its density (`getNativeSurface()`).
fn report_native(env: &mut JNIEnv<'_>, state: &State, view: &GlobalRef, width: u32, height: u32) {
    let density = (|| -> nsh::jni::errors::Result<f32> {
        let resources = env
            .call_method(
                view.as_obj(),
                "getResources",
                "()Landroid/content/res/Resources;",
                &[],
            )?
            .l()?;
        let metrics = env
            .call_method(
                &resources,
                "getDisplayMetrics",
                "()Landroid/util/DisplayMetrics;",
                &[],
            )?
            .l()?;
        env.get_field(&metrics, "density", "F")?.f()
    })()
    .unwrap_or_else(|_| {
        clear_exception(env);
        1.0
    });
    let scale = f64::from(density).max(f64::MIN_POSITIVE);
    canvas_context::set_native(
        state.context_id,
        state.window.map_or(0, |w| w.as_ptr() as usize),
        f64::from(width) / scale,
        f64::from(height) / scale,
        scale,
    );
}

/// The backing view, added to the container at its size.
fn make_child(
    env: &mut JNIEnv<'_>,
    state: &State,
    kind: Backing,
) -> nsh::jni::errors::Result<GlobalRef> {
    let class = match kind {
        Backing::TextureView => "android/view/TextureView",
        Backing::SurfaceView => "android/view/SurfaceView",
    };
    let view = env.new_object(
        class,
        "(Landroid/content/Context;)V",
        &[JValue::Object(state.android_context.as_obj())],
    )?;
    env.call_method(
        state.container.as_obj(),
        "addView",
        "(Landroid/view/View;II)V",
        &[
            JValue::Object(&view),
            JValue::Int(MATCH_PARENT),
            JValue::Int(MATCH_PARENT),
        ],
    )?;
    env.new_global_ref(view)
}

/// The view's valid `Surface`, if it has one now. A TextureView's is made
/// from its `SurfaceTexture` (once per texture, kept in `state.texture`).
/// A SurfaceView's that turned valid after being invalid is a new window:
/// the old attachment goes first.
fn current_surface(
    env: &mut JNIEnv<'_>,
    state: &mut State,
    kind: Backing,
    view: &GlobalRef,
) -> nsh::jni::errors::Result<Option<GlobalRef>> {
    match kind {
        Backing::SurfaceView => {
            let holder = holder(env, view)?;
            let surface = env
                .call_method(&holder, "getSurface", "()Landroid/view/Surface;", &[])?
                .l()?;
            if surface.is_null() {
                return Ok(None);
            }
            let valid = env.call_method(&surface, "isValid", "()Z", &[])?.z()?;
            Ok(if valid {
                Some(env.new_global_ref(surface)?)
            } else {
                None
            })
        }
        Backing::TextureView => {
            let texture = env
                .call_method(
                    view.as_obj(),
                    "getSurfaceTexture",
                    "()Landroid/graphics/SurfaceTexture;",
                    &[],
                )?
                .l()?;
            if texture.is_null() {
                return Ok(None);
            }
            if let Some((known, surface)) = &state.texture
                && env.is_same_object(known.as_obj(), &texture)?
            {
                return Ok(Some(surface.clone()));
            }
            // A new SurfaceTexture: the old window is gone with the old one.
            state.detach(env);
            let surface = env.new_object(
                "android/view/Surface",
                "(Landroid/graphics/SurfaceTexture;)V",
                &[JValue::Object(&texture)],
            )?;
            let surface = env.new_global_ref(surface)?;
            state.texture = Some((env.new_global_ref(texture)?, surface.clone()));
            Ok(Some(surface))
        }
    }
}

/// Makes the `wgpu::Surface` from `surface`'s window and attaches it.
fn attach(env: &mut JNIEnv<'_>, state: &mut State, surface: &JObject<'_>, width: u32, height: u32) {
    // SAFETY: a live `Surface`; the window comes with a reference of ours.
    let window = unsafe { ANativeWindow_fromSurface(env.get_raw(), surface.as_raw()) };
    let Some(window) = NonNull::new(window) else {
        return;
    };
    let handle = wgpu::rwh::AndroidNdkWindowHandle::new(window);
    // SAFETY: the window lives while we hold our reference, which `detach`
    // lets go only after the surface is detached (the swapchain holds its
    // own reference past that).
    let made = unsafe {
        crate::instance::instance().create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Android(
                wgpu::rwh::AndroidDisplayHandle::new(),
            )),
            raw_window_handle: wgpu::rwh::RawWindowHandle::AndroidNdk(handle),
        })
    };
    match made {
        Ok(made) => {
            let old = state.window.replace(window);
            // The new surface replaces the one on the old window first
            // (`views::attach` drops it), then the old window goes.
            views::attach(state.context_id, made, width, height);
            state.attached = Some((state.context_id, width, height));
            if let Some(old) = old {
                // SAFETY: our earlier reference; no surface uses it any more.
                unsafe { ANativeWindow_release(old.as_ptr()) };
            }
        }
        Err(err) => {
            // SAFETY: the reference taken above.
            unsafe { ANativeWindow_release(window.as_ptr()) };
            log::error!("webgpu.canvas: cannot make a surface: {err}");
        }
    }
}
