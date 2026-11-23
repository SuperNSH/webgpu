//! The process's one `wgpu::Instance`, as Dawn's `wgpu::Instance` is
//! RNWebGPU's: every adapter, and every surface the canvas makes, comes
//! from it. Backends follow wgpu's defaults for the platform (Metal on
//! Apple, Vulkan/DX12 on Windows, Vulkan/GL on Linux), and `WGPU_BACKEND`
//! and the other `WGPU_*` variables override them.
//!
//! Android takes one backend per instance ([`android_backends`]): Vulkan
//! when it yields an adapter, else GL.

use std::sync::LazyLock;

static INSTANCE: LazyLock<wgpu::Instance> = LazyLock::new(|| {
    #[allow(unused_mut)]
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    #[cfg(target_os = "android")]
    if let Some(backends) = android_backends(std::env::var_os("WGPU_BACKEND").is_some(), || {
        let mut probe = wgpu::InstanceDescriptor::new_without_display_handle();
        probe.backends = wgpu::Backends::VULKAN;
        let vulkan = wgpu::Instance::new(probe);
        !pollster::block_on(vulkan.enumerate_adapters(wgpu::Backends::VULKAN)).is_empty()
    }) {
        desc.backends = backends;
    }
    wgpu::Instance::new(desc)
});

/// The instance.
pub fn instance() -> &'static wgpu::Instance {
    &INSTANCE
}

/// The backends of Android's instance: `None` (wgpu's from the
/// environment) when `WGPU_BACKEND` is set, else Vulkan when
/// `vulkan_has_adapter` says it yields an adapter, else GL. Never both.
///
/// An `ANativeWindow` takes one producer API. A surface made for Vulkan and
/// GL at once connects the window for Vulkan (`vkCreateAndroidSurfaceKHR`),
/// so GL's EGL surface then fails (`EGL_BAD_ALLOC`, "Invalid surface"), and
/// on an emulator, whose Vulkan driver gives wgpu no adapter, GL is the only
/// adapter there is. Vulkan is the reference's (Dawn's) choice when it works.
/// Not Android-only so the host's tests cover it; `vulkan_has_adapter` runs
/// only when its answer decides.
pub fn android_backends(
    env_backend_set: bool,
    vulkan_has_adapter: impl FnOnce() -> bool,
) -> Option<wgpu::Backends> {
    if env_backend_set {
        return None;
    }
    Some(if vulkan_has_adapter() {
        wgpu::Backends::VULKAN
    } else {
        wgpu::Backends::GL
    })
}

#[cfg(test)]
mod tests {
    use super::android_backends;

    #[test]
    fn android_takes_one_backend() {
        assert_eq!(
            android_backends(false, || true),
            Some(wgpu::Backends::VULKAN)
        );
        assert_eq!(android_backends(false, || false), Some(wgpu::Backends::GL));
    }

    #[test]
    fn wgpu_backend_decides_when_set() {
        assert_eq!(
            android_backends(true, || panic!("probed though WGPU_BACKEND is set")),
            None
        );
    }
}
