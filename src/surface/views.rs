//! The canvas views on screen, by context id: the contract between the
//! platform files (`macos.rs`, `android.rs`, ...), which attach a view's
//! surface, and the canvas context (`api/canvas_context.rs`), which
//! configures and presents it.
//!
//! A `wgpu::Surface<'static>` is `Send + Sync`, so the UI thread that makes
//! the view attaches it and the JS thread uses it; the raw handles it was
//! made from never leave the platform file.
//!
//! Surfaces only: a view with no surface of its own (Linux's picture)
//! registers with [`super::offscreen::show`] instead, and the canvas context
//! asks both (docs/architecture.md "Canvas").

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
};

/// A canvas view's surface and its size in physical pixels.
#[derive(Clone)]
pub struct View {
    /// The surface the view presents.
    pub surface: Arc<wgpu::Surface<'static>>,
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
}

static VIEWS: Mutex<BTreeMap<u32, View>> = Mutex::new(BTreeMap::new());

fn views() -> MutexGuard<'static, BTreeMap<u32, View>> {
    VIEWS.lock().unwrap_or_else(|e| e.into_inner())
}

/// The view of context `context_id` shows `surface`, `width` × `height`
/// physical pixels (replacing one attached before).
pub fn attach(context_id: u32, surface: wgpu::Surface<'static>, width: u32, height: u32) {
    let replaced = views().insert(
        context_id,
        View {
            surface: Arc::new(surface),
            width,
            height,
        },
    );
    // Dropped outside the lock, as `detach` drops: a surface's drop reaches
    // the platform (and may reach this table again through a callback).
    drop(replaced);
}

/// The view of context `context_id` is now `width` × `height`.
pub fn resize(context_id: u32, width: u32, height: u32) {
    if let Some(view) = views().get_mut(&context_id) {
        view.width = width;
        view.height = height;
    }
}

/// The view of context `context_id` went: its surface goes (unless a frame
/// in flight still holds it). Platform files call
/// [`crate::api::canvas_context::view_gone`], which ends that frame first
/// and then calls this.
pub fn detach(context_id: u32) {
    let view = take(context_id);
    // Dropped outside the lock.
    drop(view);
}

/// Removes the view of `context_id` and gives it to the caller to drop.
pub(crate) fn take(context_id: u32) -> Option<View> {
    views().remove(&context_id)
}

/// The view of context `context_id`, if one is attached.
pub fn get(context_id: u32) -> Option<View> {
    views().get(&context_id).cloned()
}
