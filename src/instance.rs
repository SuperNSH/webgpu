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
    #[cfg(windows)]
    if dx12_only() {
        desc.backends = wgpu::Backends::DX12;
    }
    wgpu::Instance::new(desc)
});

/// Whether the instance is DX12's alone: on Windows, when it yields an
/// adapter (Dawn's choice there) and `WGPU_BACKEND` does not say otherwise.
/// A canvas is then a composition visual, with transparency, which only
/// DX12 presents to; else a child window (`surface/windows.rs`).
#[cfg(windows)]
pub fn dx12_only() -> bool {
    static DX12: LazyLock<bool> = LazyLock::new(|| {
        if std::env::var_os("WGPU_BACKEND").is_some() {
            return false;
        }
        let mut probe = wgpu::InstanceDescriptor::new_without_display_handle();
        probe.backends = wgpu::Backends::DX12;
        let dx12 = wgpu::Instance::new(probe);
        !pollster::block_on(dx12.enumerate_adapters(wgpu::Backends::DX12)).is_empty()
    });
    *DX12
}

/// The instance.
pub fn instance() -> &'static wgpu::Instance {
    &INSTANCE
}

/// `requestAdapter`: the adapter for `options` (no compatible surface:
/// nsh-webgpu's surfaces work with any adapter of the instance).
///
/// Where the instance may have GL over EGL (Linux, Android), adapters are
/// enumerated once and kept. Every GL adapter wgpu-hal exposes uses the
/// instance's one EGL context but locks it apart from the others, so
/// enumerating again while another thread enumerates, drops an adapter or
/// uses a GL device makes the context current on two threads at once, and
/// wgpu-hal panics (`EGL_BAD_ACCESS`, `egl.rs` `make_current`). Asking for
/// an adapter twice is common (each `requestAdapter` is a worker thread's
/// job here), and one kept adapter has one lock.
pub fn request_adapter(options: &RequestAdapter) -> Option<wgpu::Adapter> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use std::sync::{Mutex, PoisonError};
        static ADAPTERS: Mutex<Vec<wgpu::Adapter>> = Mutex::new(Vec::new());
        let mut adapters = ADAPTERS.lock().unwrap_or_else(PoisonError::into_inner);
        if adapters.is_empty() {
            *adapters = pollster::block_on(instance().enumerate_adapters(wgpu::Backends::all()));
        }
        let types: Vec<_> = adapters.iter().map(|a| a.get_info().device_type).collect();
        choose(&types, options).map(|index| adapters[index].clone())
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        pollster::block_on(instance().request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: options.power_preference,
            force_fallback_adapter: options.force_fallback_adapter,
            compatible_surface: None,
        }))
        .ok()
    }
}

/// What [`request_adapter`] takes of `GPURequestAdapterOptions`.
#[derive(Debug, Default, Clone, Copy)]
pub struct RequestAdapter {
    /// `powerPreference`.
    pub power_preference: wgpu::PowerPreference,
    /// `forceFallbackAdapter`: a software adapter or none.
    pub force_fallback_adapter: bool,
}

/// The index of wgpu-core's choice among adapters of these types, in the
/// order they were enumerated (`Instance::request_adapter`, wgpu-core 29).
/// Not Linux-only so the host's tests cover it.
pub fn choose(types: &[wgpu::DeviceType], options: &RequestAdapter) -> Option<usize> {
    use wgpu::{DeviceType as T, PowerPreference as P};
    let order = |t: T| match (t, options.power_preference) {
        (_, P::None) => 0,
        (T::IntegratedGpu, P::LowPower) | (T::DiscreteGpu, P::HighPerformance) => 1,
        (T::DiscreteGpu, P::LowPower) | (T::IntegratedGpu, P::HighPerformance) => 2,
        (T::Other, _) => 3,
        (T::VirtualGpu, _) => 4,
        (T::Cpu, _) => 5,
    };
    types
        .iter()
        .enumerate()
        .filter(|(_, t)| !options.force_fallback_adapter || **t == T::Cpu)
        .min_by_key(|(_, t)| order(**t))
        .map(|(index, _)| index)
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
    use wgpu::{DeviceType as T, PowerPreference as P};

    use super::{RequestAdapter, android_backends, choose};

    fn options(power_preference: P, force_fallback_adapter: bool) -> RequestAdapter {
        RequestAdapter {
            power_preference,
            force_fallback_adapter,
        }
    }

    #[test]
    fn chooses_as_wgpu_core_does() {
        let types = [T::Cpu, T::IntegratedGpu, T::DiscreteGpu, T::Other];
        assert_eq!(choose(&types, &options(P::None, false)), Some(0));
        assert_eq!(choose(&types, &options(P::LowPower, false)), Some(1));
        assert_eq!(choose(&types, &options(P::HighPerformance, false)), Some(2));
        // The first of equals, as a stable sort leaves it.
        let twice = [T::Cpu, T::Cpu];
        assert_eq!(choose(&twice, &options(P::LowPower, false)), Some(0));
        assert_eq!(choose(&types[1..], &options(P::None, true)), None);
        assert_eq!(choose(&types, &options(P::HighPerformance, true)), Some(0));
        assert_eq!(choose(&[], &options(P::None, false)), None);
    }

    /// Threads asking at once, each dropping its adapter, as the pool's
    /// `requestAdapter` jobs do (EGL's BAD_ACCESS panic on Linux).
    #[test]
    fn many_threads_can_ask_at_once() {
        std::thread::scope(|scope| {
            for _ in 0..16 {
                scope.spawn(|| {
                    for _ in 0..8 {
                        drop(super::request_adapter(&RequestAdapter::default()));
                    }
                });
            }
        });
    }

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
