//! Windows: the canvas is a child window of its own (`NativeComponent::window`)
//! whose `HWND` becomes the `wgpu::Surface` (`SurfaceTargetUnsafe::RawHandle`,
//! Win32), for DX12 or Vulkan alike.
//!
//! NSH makes the window a child of the window its view shows in, moves and
//! clips it with the view after every frame and hides it while the view is
//! not drawn (`nsh-platform-windows` `native.rs`, `adopt`, `Placement`).
//! The window answers `WM_NCHITTEST` with `HTTRANSPARENT`, so presses and
//! moves over the canvas reach NSH's window and the `<Canvas>`'s own
//! handlers, as on the other platforms. Being a window, it sits above
//! whatever NSH draws over it (the "airspace" limit of child windows; a
//! DirectComposition visual, `NativeComponent::visual` with
//! `SurfaceTargetUnsafe::CompositionVisual`, would lift it but ties the
//! canvas to DX12: see docs/parity/surfaces-other.md).
//!
//! The view's props: `contextId` (the canvas context's id, set once). The
//! surface is attached ([`super::views::attach`]) when the context id
//! arrives, resized on `WM_SIZE` and detached when the component goes.

use std::{
    cell::Cell,
    num::NonZeroIsize,
    rc::Rc,
    sync::{
        Once,
        atomic::{AtomicU32, Ordering},
    },
};

use nsh_core::{JsonValue, prelude::*};
use nsh_platform_windows::{
    ComponentContext, Components, NativeComponent,
    windows::{
        Win32::{
            Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
            System::LibraryLoader::GetModuleHandleW,
            UI::HiDpi::GetDpiForWindow,
            UI::WindowsAndMessaging::{
                CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow,
                GWLP_USERDATA, GetClientRect, GetWindowLongPtrW, HTTRANSPARENT, IDC_ARROW,
                LoadCursorW, RegisterClassW, SetWindowLongPtrW, WM_ERASEBKGND, WM_NCHITTEST,
                WM_SIZE, WNDCLASSW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_NOPARENTNOTIFY,
                WS_POPUP,
            },
        },
        core::w,
    },
};

use super::views;
use crate::api::canvas_context;

/// The name `<Canvas>` makes its view with (`nativeComponent("webgpu.canvas")`).
pub const COMPONENT: &str = "webgpu.canvas";

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

/// The canvas view: a window, and the context id it presents for.
struct Canvas {
    hwnd: Option<HWND>,
    context_id: Cell<u32>,
    _context: ComponentContext,
}

impl Canvas {
    fn new(context: ComponentContext) -> Self {
        let hwnd = make_window();
        if hwnd.is_none() {
            log::error!("webgpu.canvas: cannot create the canvas window");
        }
        Self {
            hwnd,
            context_id: Cell::new(NO_CONTEXT),
            _context: context,
        }
    }

    /// Makes and attaches the surface for `context_id`.
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
            canvas_context::view_gone(old);
        }
        if id != NO_CONTEXT {
            self.attach(id);
        }
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        let id = self.context_id.replace(NO_CONTEXT);
        if id != NO_CONTEXT {
            canvas_context::view_gone(id);
        }
        if let Some(hwnd) = self.hwnd.take() {
            // SAFETY: our window, on the thread that made it (the UI thread
            // components live on).
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
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
