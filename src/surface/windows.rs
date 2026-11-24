//! Windows: the canvas is a DirectComposition visual (`NativeComponent::visual`)
//! showing a composition surface handle that the `wgpu::Surface` presents to
//! (`SurfaceTargetUnsafe::SurfaceHandle`, DX12). NSH stacks the visual in
//! order with what it draws, so a canvas can be transparent
//! (`alphaMode: "premultiplied"`) and NSH draws over it.
//!
//! The handle, not `SurfaceTargetUnsafe::CompositionVisual`: the visual's
//! content is set once, on the UI thread, and a swap chain made later on
//! the JS thread shows in it without a commit of its own. A visual shows
//! its content 1:1, so it is scaled from the drawing buffer's size to the
//! view's ([`buffer_size`]).
//!
//! Without a DX12 adapter (or with `WGPU_BACKEND` set) the canvas is a child
//! window of its own (`NativeComponent::window`) whose `HWND` becomes the
//! surface (Vulkan, GL): opaque, and above whatever NSH draws over it. Its
//! `WM_NCHITTEST` answers `HTTRANSPARENT`, so presses over it reach NSH's
//! window and the `<Canvas>`'s own handlers, as they do over a visual.
//!
//! The view's props: `contextId` (the canvas context's id, set once). The
//! surface is attached ([`super::views::attach`]) when the context id and,
//! for a visual, its first placement arrive, resized with it and detached
//! when the component goes.

use std::{
    cell::Cell,
    collections::BTreeMap,
    num::NonZeroIsize,
    rc::Rc,
    sync::{
        Mutex, MutexGuard, Once, PoisonError, Weak,
        atomic::{AtomicU32, Ordering},
    },
};

use nsh_core::{JsonValue, prelude::*};
use nsh_platform_windows::{
    ComponentContext, Components, NativeComponent, Placement,
    windows::{
        Win32::{
            Foundation::{CloseHandle, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
            Graphics::DirectComposition::{
                DCompositionCreateSurfaceHandle, IDCompositionDesktopDevice,
                IDCompositionRectangleClip, IDCompositionVisual2, IDCompositionVisual3,
            },
            System::LibraryLoader::GetModuleHandleW,
            UI::HiDpi::{GetDpiForSystem, GetDpiForWindow},
            UI::WindowsAndMessaging::{
                CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow,
                GWLP_USERDATA, GetClientRect, GetWindowLongPtrW, HTTRANSPARENT, IDC_ARROW,
                LoadCursorW, RegisterClassW, SetWindowLongPtrW, WM_ERASEBKGND, WM_NCHITTEST,
                WM_SIZE, WNDCLASSW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_NOPARENTNOTIFY,
                WS_POPUP,
            },
        },
        core::{Interface, w},
    },
};
use windows_numerics::Matrix3x2;

use super::views;
use crate::api::canvas_context;

/// The name `<Canvas>` makes its view with (`nativeComponent("webgpu.canvas")`).
pub const COMPONENT: &str = "webgpu.canvas";

/// `dcomp.h`'s (not in the `windows` crate): read and write.
const COMPOSITIONOBJECT_ALL_ACCESS: u32 = 0x3;

/// No context yet: the window's user data until `contextId` arrives
/// (context ids start at 1).
const NO_CONTEXT: u32 = 0;

#[native_components(windows)]
fn components(components: &Components) {
    components.register(
        COMPONENT,
        Rc::new(|context: ComponentContext| -> Box<dyn NativeComponent> {
            Box::new(Canvas::new(context))
        }),
    );
}

/// How many canvases attached a surface (diagnostics, tests).
static ATTACHED: AtomicU32 = AtomicU32::new(0);

/// The visuals a canvas is composed with: the clip and opacity of its
/// ancestors (`outer`), its place and corners (`inner`), its frames scaled
/// to the view (`content`).
#[derive(Clone)]
struct Visuals {
    device: IDCompositionDesktopDevice,
    outer: IDCompositionVisual2,
    inner: IDCompositionVisual2,
    corners: IDCompositionRectangleClip,
    content: IDCompositionVisual2,
    /// What the surface presents to.
    handle: HANDLE,
}

impl Visuals {
    fn new(device: &IDCompositionDesktopDevice) -> windows::core::Result<Self> {
        // SAFETY: plain calls on the live device and what it makes here.
        unsafe {
            let handle = DCompositionCreateSurfaceHandle(COMPOSITIONOBJECT_ALL_ACCESS, None)?;
            let made = (|| {
                let outer = device.CreateVisual()?;
                let inner = device.CreateVisual()?;
                let content = device.CreateVisual()?;
                let corners = device.CreateRectangleClip()?;
                content.SetContent(&device.CreateSurfaceFromHandle(handle)?)?;
                inner.SetClip(&corners)?;
                inner.AddVisual(&content, true, None)?;
                outer.AddVisual(&inner, true, None)?;
                Ok((outer, inner, corners, content))
            })();
            match made {
                Ok((outer, inner, corners, content)) => Ok(Self {
                    device: device.clone(),
                    outer,
                    inner,
                    corners,
                    content,
                    handle,
                }),
                Err(err) => {
                    let _ = CloseHandle(handle);
                    Err(err)
                }
            }
        }
    }

    /// Puts the canvas at `at` (NSH commits the window's composition after).
    fn place(&self, at: &Placement) {
        let c = &self.corners;
        let [tl, tr, br, bl] = at.radii;
        // SAFETY: plain calls on the live visuals.
        unsafe {
            let _ = self.inner.SetTransform2(&at.matrix);
            let _ = c.SetLeft2(0.0);
            let _ = c.SetTop2(0.0);
            let _ = c.SetRight2(at.size.0 as f32);
            let _ = c.SetBottom2(at.size.1 as f32);
            let _ = c.SetTopLeftRadiusX2(tl);
            let _ = c.SetTopLeftRadiusY2(tl);
            let _ = c.SetTopRightRadiusX2(tr);
            let _ = c.SetTopRightRadiusY2(tr);
            let _ = c.SetBottomRightRadiusX2(br);
            let _ = c.SetBottomRightRadiusY2(br);
            let _ = c.SetBottomLeftRadiusX2(bl);
            let _ = c.SetBottomLeftRadiusY2(bl);
            let _ = self.outer.SetClip2(&at.clip);
            if let Ok(outer) = self.outer.cast::<IDCompositionVisual3>() {
                let _ = outer.SetOpacity2(at.alpha.clamp(0.0, 1.0));
            }
        }
    }

    /// Scales a `buffer` pixels' frame to a `view` pixels' view.
    fn scale(&self, view: (u32, u32), buffer: (u32, u32)) {
        let matrix = Matrix3x2 {
            M11: view.0 as f32 / buffer.0.max(1) as f32,
            M12: 0.0,
            M21: 0.0,
            M22: view.1 as f32 / buffer.1.max(1) as f32,
            M31: 0.0,
            M32: 0.0,
        };
        // SAFETY: a plain call on the live visual.
        let _ = unsafe { self.content.SetTransform2(&matrix) };
    }
}

/// A composed canvas on screen, by context id: what the JS thread scales
/// when the drawing buffer's size changes.
struct Composed {
    visuals: Visuals,
    view: (u32, u32),
    buffer: (u32, u32),
}

// SAFETY: DirectComposition's device and visuals are free-threaded ("Basic
// concepts": its interfaces take calls from any thread); the handle is a
// number.
unsafe impl Send for Composed {}

static COMPOSED: Mutex<BTreeMap<u32, Composed>> = Mutex::new(BTreeMap::new());

fn composed() -> MutexGuard<'static, BTreeMap<u32, Composed>> {
    COMPOSED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The drawing buffer of context `context_id` is now `width` × `height`
/// (on the thread that configured it): a composed canvas scales to its view.
pub fn buffer_size(context_id: u32, width: u32, height: u32) {
    let mut all = composed();
    let Some(canvas) = all.get_mut(&context_id) else {
        return;
    };
    if std::mem::replace(&mut canvas.buffer, (width, height)) != (width, height) {
        canvas.visuals.scale(canvas.view, canvas.buffer);
        // SAFETY: a plain call on the live device.
        let _ = unsafe { canvas.visuals.device.Commit() };
    }
}

/// A surface handle (as a number) whose canvas went, and the surface that
/// may still be in a frame on the JS thread.
type Closing = (isize, Weak<wgpu::Surface<'static>>);

/// Handles to close once their surface went: wgpu needs a handle for as long
/// as its surface lives.
static CLOSING: Mutex<Vec<Closing>> = Mutex::new(Vec::new());

/// Closes the handles whose surface went, after adding `more`.
fn close_handles(more: Option<Closing>) {
    let mut closing = CLOSING.lock().unwrap_or_else(PoisonError::into_inner);
    closing.extend(more);
    closing.retain(|(handle, surface)| {
        if surface.strong_count() > 0 {
            return true;
        }
        // SAFETY: a handle we made, which nothing uses any more.
        let _ = unsafe { CloseHandle(HANDLE(*handle as *mut core::ffi::c_void)) };
        false
    });
}

/// The canvas view: its visuals or its window, and the context id it
/// presents for.
struct Canvas {
    hwnd: Option<HWND>,
    /// Whether it is a visual (decided when made), and it once NSH asked.
    composed: bool,
    visuals: Option<Visuals>,
    placed: Option<(u32, u32)>,
    surface: Weak<wgpu::Surface<'static>>,
    context_id: Cell<u32>,
    _context: ComponentContext,
}

impl Canvas {
    fn new(context: ComponentContext) -> Self {
        let composed = crate::instance::dx12_only();
        let hwnd = if composed { None } else { make_window() };
        if hwnd.is_none() && !composed {
            log::error!("webgpu.canvas: cannot create the canvas window");
        }
        Self {
            hwnd,
            composed,
            visuals: None,
            placed: None,
            surface: Weak::new(),
            context_id: Cell::new(NO_CONTEXT),
            _context: context,
        }
    }

    /// Makes and attaches the surface of a composed canvas once it has its
    /// context, visuals and place; does nothing when it did.
    fn attach_visual(&mut self) {
        let context_id = self.context_id.get();
        let (Some(visuals), Some(size)) = (&self.visuals, self.placed) else {
            return;
        };
        if context_id == NO_CONTEXT || self.surface.strong_count() > 0 {
            return;
        }
        // SAFETY: the handle is closed after the surface went (`close_handles`).
        let surface = unsafe {
            crate::instance::instance()
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::SurfaceHandle(visuals.handle.0))
        };
        match surface {
            Ok(surface) => {
                composed().insert(
                    context_id,
                    Composed {
                        visuals: visuals.clone(),
                        view: size,
                        buffer: (0, 0),
                    },
                );
                views::attach(context_id, surface, size.0, size.1);
                if let Some(view) = views::get(context_id) {
                    self.surface = std::sync::Arc::downgrade(&view.surface);
                }
                report_visual(visuals, context_id, size);
                canvas_context::surface_attached(context_id);
                ATTACHED.fetch_add(1, Ordering::Relaxed);
            }
            Err(err) => log::error!("webgpu.canvas: cannot make a surface: {err}"),
        }
    }

    /// The canvas of `context_id` goes: its context detaches.
    fn detach(&mut self, context_id: u32) {
        canvas_context::view_gone(context_id);
        composed().remove(&context_id);
    }

    /// Makes and attaches the surface for `context_id` on the window.
    fn attach(&self, context_id: u32) {
        let Some(hwnd) = self.hwnd else {
            return;
        };
        let Some(handle) = NonZeroIsize::new(hwnd.0 as isize) else {
            return;
        };
        let mut window = wgpu::rwh::Win32WindowHandle::new(handle);
        // SAFETY: the module handle of this process.
        let instance = unsafe { GetModuleHandleW(None) }.ok();
        window.hinstance = instance.and_then(|h| NonZeroIsize::new(h.0 as isize));
        // SAFETY: the window lives until `Drop`, which detaches the surface
        // first (the context's own clones of the `Arc` may outlive that by
        // a frame: presenting to a destroyed window fails with a surface
        // error, it does not crash).
        let surface = unsafe {
            crate::instance::instance().create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Windows(
                        wgpu::rwh::WindowsDisplayHandle::new(),
                    )),
                    raw_window_handle: wgpu::rwh::RawWindowHandle::Win32(window),
                },
            )
        };
        match surface {
            Ok(surface) => {
                let (width, height) = client_size(hwnd);
                // SAFETY: a plain call on our live window.
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, context_id as isize);
                }
                views::attach(context_id, surface, width, height);
                report_native(hwnd, context_id, width, height);
                canvas_context::surface_attached(context_id);
                ATTACHED.fetch_add(1, Ordering::Relaxed);
            }
            Err(err) => log::error!("webgpu.canvas: cannot make a surface: {err}"),
        }
    }
}

impl NativeComponent for Canvas {
    fn window(&self) -> Option<HWND> {
        self.hwnd
    }

    fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
        if name != "contextId" {
            return;
        }
        let id = value
            .and_then(JsonValue::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(NO_CONTEXT);
        let old = self.context_id.replace(id);
        if old == id {
            return;
        }
        if old != NO_CONTEXT {
            self.detach(old);
        }
        if id != NO_CONTEXT {
            if self.composed {
                self.attach_visual();
            } else {
                self.attach(id);
            }
        }
    }

    fn composed(&self) -> bool {
        self.composed
    }

    fn visual(&mut self, device: &IDCompositionDesktopDevice) -> Option<IDCompositionVisual2> {
        match Visuals::new(device) {
            Ok(visuals) => {
                let outer = visuals.outer.clone();
                self.visuals = Some(visuals);
                Some(outer)
            }
            Err(err) => {
                log::error!("webgpu.canvas: cannot make its visual: {err}");
                None
            }
        }
    }

    fn placed(&mut self, at: Option<Placement>) {
        let (Some(at), Some(visuals)) = (at, &self.visuals) else {
            return;
        };
        visuals.place(&at);
        let size = (
            u32::try_from(at.size.0).unwrap_or(0).max(1),
            u32::try_from(at.size.1).unwrap_or(0).max(1),
        );
        if self.placed.replace(size) == Some(size) {
            return;
        }
        let context_id = self.context_id.get();
        if self.surface.strong_count() == 0 {
            self.attach_visual();
            return;
        }
        views::resize(context_id, size.0, size.1);
        report_visual(visuals, context_id, size);
        if let Some(canvas) = composed().get_mut(&context_id) {
            canvas.view = size;
            canvas.visuals.scale(size, canvas.buffer);
        }
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        let id = self.context_id.replace(NO_CONTEXT);
        if id != NO_CONTEXT {
            self.detach(id);
        }
        let visuals = self.visuals.take();
        close_handles(visuals.map(|v| (v.handle.0 as isize, std::mem::take(&mut self.surface))));
        if let Some(hwnd) = self.hwnd.take() {
            // SAFETY: our window, on the thread that made it (the UI thread
            // components live on).
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

/// Tells the canvas context the visual, its size in points (pixels at the
/// system's DPI over 96: a visual has no window to ask) and that scale.
fn report_visual(visuals: &Visuals, context_id: u32, size: (u32, u32)) {
    // SAFETY: a plain call.
    let scale = f64::from(unsafe { GetDpiForSystem() }.max(96)) / 96.0;
    canvas_context::set_native(
        context_id,
        visuals.outer.as_raw() as usize,
        f64::from(size.0) / scale,
        f64::from(size.1) / scale,
        scale,
    );
}

/// The window's client size in pixels (at least 1 × 1).
fn client_size(hwnd: HWND) -> (u32, u32) {
    let mut rect = RECT::default();
    // SAFETY: a plain call on a live window.
    let _ = unsafe { GetClientRect(hwnd, &mut rect) };
    (
        u32::try_from(rect.right - rect.left).unwrap_or(0).max(1),
        u32::try_from(rect.bottom - rect.top).unwrap_or(0).max(1),
    )
}

/// Tells the canvas context the window's handle, its size in points
/// (pixels at the window's DPI over 96) and that scale (`getNativeSurface()`).
fn report_native(hwnd: HWND, context_id: u32, width: u32, height: u32) {
    // SAFETY: a plain call on our live window.
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let scale = if dpi == 0 { 1.0 } else { f64::from(dpi) / 96.0 };
    canvas_context::set_native(
        context_id,
        hwnd.0 as usize,
        f64::from(width) / scale,
        f64::from(height) / scale,
        scale,
    );
}

/// The canvas window's procedure: size changes go to [`views::resize`],
/// hit tests fall through to NSH's window, and nothing is erased (the GPU
/// draws every pixel).
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_SIZE => {
            // SAFETY: a plain call on the window this procedure serves.
            let context_id = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as u32;
            if context_id != NO_CONTEXT {
                let width = (lparam.0 & 0xffff) as u32;
                let height = ((lparam.0 >> 16) & 0xffff) as u32;
                views::resize(context_id, width.max(1), height.max(1));
                report_native(hwnd, context_id, width.max(1), height.max(1));
            }
            LRESULT(0)
        }
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_ERASEBKGND => LRESULT(1),
        // SAFETY: the default procedure, with what we were given.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// A hidden popup window of the canvas class (NSH re-styles it as a child
/// and parents it).
fn make_window() -> Option<HWND> {
    static REGISTER: Once = Once::new();
    let class = w!("NshWebGpuCanvas");
    // SAFETY: the module handle of this process.
    let instance: HINSTANCE = unsafe { GetModuleHandleW(None) }.ok()?.into();
    REGISTER.call_once(|| {
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            // SAFETY: a system cursor.
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
            lpszClassName: class,
            ..Default::default()
        };
        // SAFETY: the class lives as long as the process (static name).
        if unsafe { RegisterClassW(&wc) } == 0 {
            log::error!("webgpu.canvas: cannot register the window class");
        }
    });
    // SAFETY: a plain window of the class registered above.
    unsafe {
        CreateWindowExW(
            WS_EX_NOPARENTNOTIFY,
            class,
            w!(""),
            WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(instance),
            None,
        )
    }
    .ok()
}
