# Parity: surfaces-other (Windows, Linux, Android, web, offscreen)

Area (d). Files: `src/surface/{windows,linux,android,offscreen}.rs`, `tsx/surface/web.tsx`,
the area (d) block of `Cargo.toml`, `tests/surfaces_other/`,
`examples/headless/surfaces-other.test.tsx`, `examples/canvas-native/`, `examples/canvas-web/`. Evidence:
`docs/evidence/surfaces-other/`.

> Area notes from phase 2. The status of every export is in [docs/parity.md](../parity.md), which is current;
> rows here that phase 3 (integration) changed are updated and marked "phase 3".

Every platform registers the native component `webgpu.canvas` (prop `contextId`, as `<Canvas>` gives it) and follows
the contract of `src/surface/views.rs`: make a `wgpu::Surface<'static>` from `crate::instance()` and `views::attach` /
`resize` / `detach` it by context id. Linux, which cannot have a surface, goes through `offscreen.rs` instead.

## Status per platform

| Platform | How | Status | Evidence |
|---|---|---|---|
| Android | `FrameLayout` holding a `TextureView` (default) or `SurfaceView`; `ANativeWindow_fromSurface` → `RawHandle` (AndroidNdk) | **Run on screen** (emulator, AVD `nsh-webgpu-surfaces-other`, GLES backend), TextureView and SurfaceView side by side, animated, resized, with the one-backend `instance.rs` (applied in phase 3; area (d) ran it from a scratch copy). Without it every configure fails on Android. | `android-canvas-patched.png`, `android-canvas-patched-later.png`, `android-run-patched.txt`; failure without the patch: `android-canvas-unpatched.png`, `android-run-unpatched.txt` |
| Windows | `NativeComponent::visual`: a composition surface handle (`SurfaceHandle`, DX12), scaled to the view; `placed` → `views::resize`. Without DX12, a child `HWND` from `NativeComponent::window` | **Run on screen in CI** (`docs/evidence/platforms/windows-canvas.png`) ||
| Linux | `gtk::Picture` showing offscreen frames (readback → `gdk::MemoryTexture`) | **Run on screen** in CI and in an arm64 container (Vulkan on lavapipe) | `docs/evidence/platforms/linux-canvas.png` |
| Web | the browser's own WebGPU, as the reference's `index.web.tsx`: a `<View testID="rnwgpu-canvas-<id>">` whose DOM element gets a `<canvas>` | **Run on screen** in a browser (`nsh run --target web`, TSX-only app). | `web-canvas-browser.png`, `web-run.txt` |
| wasm build of the crate | `wgpu` feature `fragile-send-sync-non-atomic-wasm` (Cargo block) | Passes clippy `-D warnings` for `wasm32-unknown-unknown` (phase 3: a wasm-only `blocking` in `api/gpu.rs`, used by `adapter.rs` and `image_bitmap.rs`). Not run. | `../evidence/integration/clippy-wasm32.txt` |
| Offscreen | `offscreen.rs`: texture-backed context + readback | **Tested** on the Mac's GPU (6 cargo tests, each seen failing on purpose). `WgpuOffscreenCanvas` is registered in `lib.rs` (phase 3), and the canvas context hands a surface-less frame to a presenting view (`offscreen::deliver`, phase 3). | `cargo-test-surfaces-other.txt`, `broken-on-purpose-{unpad,present}.txt` |

## Rows (export / behaviour, status, reason)

| Export / behaviour | Status | Reason |
|---|---|---|
| `<Canvas android={{ backingView: "TextureView" }}>` | implemented (prop `androidBackingView`) | |
| `backingView: "SurfaceView"`, `opaque`, `zOrderOnTop`, `zOrderMediaOverlay` | implemented (`androidOpaque`, `androidZOrderOnTop`, `androidZOrderMediaOverlay`; opacity/z-order apply live, TextureView opaque off on API 29 as the reference) | |
| `backingView: "HardwareBufferView"` | falls back to a TextureView with a warning | needs the reference's Java `HardwareRenderer` path; an nshm ships no Java |
| Android surface lifecycle | followed on the UI thread's `AChoreographer` each vsync, not `SurfaceHolder.Callback` | an nshm cannot ship Java classes, so no listener interface can be implemented; a surface the system destroys is detached on the next vsync (a present racing it fails with a surface error, nothing is freed under it) |
| Windows canvas over NSH content | only without DX12: a child window sits above whatever NSH draws over it (airspace) | NSH's child-window hosting; with DX12 the canvas is a composition visual and has no such limit |
| Linux canvas | offscreen render + one GPU→CPU→GPU copy per presented frame; a frame presented while the previous readback is in flight is dropped | a GTK 4 widget has no native window / `wl_surface` of its own; wgpu 29 cannot export a texture as dmabuf (`GdkDmabufTextureBuilder` would make it zero-copy) |
| Windows `rgba16float` canvas | rendered offscreen in 16 bits and drawn onto a 10-bit swap chain (8-bit when transparent) with wgpu's `TextureBlitter`; clipped to SDR, no `toneMapping: "extended"` | DXGI takes a float swap chain's values as linear scRGB, a canvas's are sRGB-encoded |
| Linux `alphaMode` | `"opaque"` shown as `X8` memory formats, `"premultiplied"` as premultiplied | |
| Web: `navigator.gpu`, `GPU*` classes | the browser's (`browserGPU()` reads it at module evaluation) | as the reference's web entry; `installWebGPU()` is a no-op on the web (phase 3), and `<Canvas>` renders `WebCanvasView` there |
| Web: `getContext("webgpu")` | the browser's context with a no-op `present()`; drawing buffer = layout size × `devicePixelRatio` | as `WebPolyfillGPUModule.ts` |
| Web: `CanvasRef.getNativeSurface()` | `{surface: BigInt(id), width, height, clientWidth, clientHeight}` + the DOM canvas's event methods | as the reference |
| Web: canvas DOM id | `rnwgpu-canvas-<id>`; the host view carries it as `data-testid` | NSH has no `<canvas>` element and shows no native component on the web; a `testID` passed to `<Canvas>` on the web is replaced |
| `GPUOffscreenCanvas.getImageData` readback | any width (rows padded to 256 and unpadded natively) | the reference's `bytesPerRow = width * 4` only works for widths that are multiples of 64 |

## Facts found (NSH nightly 589a60f and before, wgpu 29)

- The web player runs a TSX app's modules in a same-origin iframe realm (`nsh-platform-web/web/browser/realm.js`)
  while the views are elements of the page: `globalThis.document` is the iframe's, empty. `web.tsx` uses
  `parent.document` / `parent.navigator.gpu`.
- The first `onLayout` of a view comes before the DOM runtime has inserted its element, so `web.tsx` looks for it
  again every 16 ms (up to a second).
- Android: `vkCreateAndroidSurfaceKHR` connects the `ANativeWindow` (API EGL); a `wgpu::Surface` made for Vulkan
  *and* GL then fails GL's `eglCreateWindowSurface` (`EGL_BAD_ALLOC`, "already connected"), and `configure` on a GL
  device reports "Invalid surface". The emulator has a Vulkan driver but wgpu finds no Vulkan adapter on it, so GL is
  the only adapter while the Vulkan surface still connects the window. Fix: one backend per instance on Android.
- NDK 29.0.14206865's `llvm-ar` on this Mac is a shell script that execs itself (dated Nov 9, not ours): `nsh run
  --target android` hangs in `zstd-sys`'s build script with it. `ANDROID_NDK_HOME=.../ndk/28.2.13676358` works.
- `cargo clippy --target aarch64-linux-android` failed `-D warnings` in `api/device.rs:39` and `error.rs:199`
  (`missing_const_for_thread_local` on `thread_local!`s that are already `const`; Android only). Phase 3 allows the
  lint on those two statics with the reason; clippy passes for Android.

## Existing solutions looked at for the desktops (2026-11-24)

The canvases are native views (no GPUI), and wgpu does the rendering and the presenting: `CoreAnimationLayer` on macOS,
`SurfaceHandle` on Windows. NSH's `#[native_components]` supplies the view, the window or the visual.

| Looked at | Verdict |
|---|---|
| NSH's own shader views on Linux (`nsh-platform-linux`, `shader.rs`) | what we do: wgpu frames read back into a `gdk::MemoryTexture` |
| [`wgpu-dmabuf`](https://crates.io/crates/wgpu-dmabuf) 0.1.0 + `gdk::DmabufTexture` | the zero-copy route for Linux, not taken: it needs wgpu 30 (`wgpu-hal =30.0.1`; NSH and we are on 29), and says it is untested on discrete GPUs and across vendors. To look at again when NSH moves to wgpu 30 |
| [`wgpu-external-frame`](https://crates.io/crates/wgpu-external-frame), [`wgpu-native-texture-interop`](https://crates.io/crates/wgpu-native-texture-interop) | described as importing other producers' frames into wgpu; the canvas needs the export. By their descriptions only |
| A Wayland subsurface under GTK's window | GTK 4 has no API to embed one, and it would not exist on X11 |
| [`raw-window-metal`](https://crates.io/crates/raw-window-metal) | makes a `CAMetalLayer` for a view that is not ours; the canvas owns its view, as the reference's `MetalView` does |
| wgpu's `Dx12SwapchainKind::DxgiFromVisual` | one visual over a whole window; the canvas needs one among NSH's own |
| [react-native-tgfx](https://github.com/margelo/react-native-tgfx) | a 2D drawing library (a Skia alternative) for iOS and Android: no WebGPU API and no desktop code. [tgfx](https://github.com/Tencent/tgfx) itself runs on the desktops, with its own Metal, D3D12 and Vulkan code, which is wgpu's part here |
| `mach-dxcompiler-rs` (wgpu's `static-dxc`) | taken, as the crate's feature `static-dxc` |
