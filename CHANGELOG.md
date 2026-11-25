# Changelog

## 0.1.0 (unreleased)

The first version: react-native-webgpu (`06599f9`) ported to NSH on wgpu 29, for NSH 0.7.

### Added

- `navigator.gpu` and every WebGPU spec class (`GPUAdapter`, `GPUDevice`, `GPUBuffer`, `GPUTexture`, the pipelines, encoders, passes, bundles, query sets, the errors and the flag constants), as TSX classes over the Rust module `nsh-webgpu/native`. `installWebGPU()` puts them on `globalThis` when the package is imported.
- Error scopes, `uncapturederror`, `device.lost`, `GPUPipelineError` from the `*Async` pipeline creators; WebIDL conversion errors as `TypeError`/`RangeError`; wgpu panics turned into `OperationError`.
- `<Canvas>` with `getContext("webgpu")`, `present()`, `getNativeSurface()` and the reference's props (`colorSpace`, `toneMapping`, `android`). Native views: a `CAMetalLayer` view on macOS and iOS, a `TextureView` or `SurfaceView` on Android, a child `HWND` on Windows, a `gtk::Picture` fed by offscreen frames on Linux, the browser's own WebGPU on the web.
- `requestAnimationFrame` / `cancelAnimationFrame` on NSH's frame clock.
- The hooks and provider: `useDevice`, `useCanvasRef`, `useSurface`, `GPUDeviceProvider`, `useMainDevice`, `warnIfNotHardwareAccelerated`, `useNativeBufferSize`, as NSH signals with the reference's names and return shapes.
- `GPUOffscreenCanvas` with `getImageData()`, which works for any width (the reference's needs a multiple of 64).
- `createImageBitmap` (PNG, JPEG, WebP bytes, `ImageData`, crop, resize, `flipY`, premultiplication), `copyExternalImageToTexture`, `importExternalTexture` from a `GPUTexture`.
- `createTestVideoFrame` (bgra8 and nv12, in memory), `getPreferredHighBitDepthCanvasFormat`, `WebGPUModule`.
- Kept by name, throwing `ERR_WEBGPU_UNSUPPORTED`: `importDevice`, `adoptTexture`, `createVideoPlayer`, `createNativeBuffer`, `createNativeBufferFromPointer`, `NativeBufferView`, `GPUTexture.nativePointer`. See `docs/parity.md`.
- Tests: cargo tests on the GPU and `nsh test` in `examples/headless`, porting the reference's specs and comparing against its snapshots. Examples: `examples/canvas-apple`, `canvas-native`, `canvas-web`.
- CI (`.github/workflows/ci.yml`), a `justfile` for the gates, the guides in `docs/guide/`.

### Known gaps

- `GPUDevice.importSharedTextureMemory`, `GPUDevice.importSharedFence` and `GPUQueue.drawElementImageToTexture` exist and throw (reject) `ERR_WEBGPU_UNSUPPORTED`.
- Android: run on an emulator (GLES) only; no physical device.
- The Stanford dragon mesh, demo data of the gallery only, has no licence beyond Stanford's acknowledgement (`NOTICE.md`).

### Fixed (phase 4c, after the audit in `docs/audit/REPORT.md`)

- `NOTICE.md` and `examples/gallery/LICENSES.md`: react-native-webgpu's MIT notices, @webgpu/types, webgpu-samples, wgpu-matrix, the gallery's assets.
- `device.destroy()` releases the device's id and listeners; a device the app drops is collected and released (weak listeners, a `FinalizationRegistry`).
- `<Canvas>` sets its ref once mounted and laid out, sets it to `null` on unmount, and `getContext()` on an unmounted canvas throws `[WebGPU] Cannot get context before mount`, as the reference's; a destroyed context id is never made again. `children` are accepted (not rendered), `NativeProps` is exported, and the change warnings are development-only, with the reference's Android ones.
- `useSurface()` follows the canvas's mount, layout, surface attach and unmount.
- The reference's `declare global` augmentations of the WebGPU types.
- `createTestVideoFrame(width, height)` has the reference export's signature.
- Overflow-checked buffer ranges, the Android window released after its surface is replaced, a replaced surface dropped outside the views lock, `Wgpu.release` guarded, a ticket dropped when its wait gives up; Cargo metadata.

### NSH 0.7

- NSH comes from crates.io, its release 0.7, instead of a nightly's commit. An app needs no Rust crate of its own: `nsh add github:SuperNSH/webgpu` is enough.
- The gallery runs in a real window on macOS, Windows and Linux in CI (`.github/workflows/onscreen.yml`: its tests with `nsh test --target`, and a picture of each page).
- A canvas drawn once is always shown: a frame whose view attached as it was drawn, or whose window was occluded when it was presented, never was (Storage Buffer Vertices was blank in one run in CI). The On screen workflow fails on a blank page (`scripts/blank.py`).
- The UI thread (a view attaching) and the JS thread configure and acquire a canvas's surface in turn. At once, the second got "Surface image is already acquired" (Android, the gallery's Surface Churn under load) or waited for a drawable forever (Metal, in a test).
- Web: `<Canvas>` sets its ref once the `<canvas>` is in the page. It came at the first layout, when `getContext()` still returned `null` (found by the gallery's Surface Churn).
- Web: `createImageBitmap` reads encoded bytes (an `ArrayBuffer` or a view) too, as its types say; the browser's takes a `Blob` only.
- Android (GLES): a buffer is read through a fresh copy each time it is mapped. The emulator's driver gave the first map's bytes again after a later copy into the buffer, with no error (found by the gallery's MNIST Inference).

### Fixed (the gallery in desktop windows)

- Windows: reconfiguring or resizing a canvas no longer fails (`ResizeBuffers`, "window is in use") while JS still holds a past frame's texture or view; they expire, as the spec says.
- Windows and Android: a canvas can be configured with one device and then another. The swap chain stayed the first device's, for which Windows removed the GPU device and every later `requestAdapter` found none.
- Android (GLES): a canvas with a usage (`COPY_SRC`), view formats or a format its surface lacks renders offscreen and is drawn onto it, instead of losing the usage or failing to configure. `rgb10a2unorm`, what `getPreferredHighBitDepthCanvasFormat()` names there, is a canvas format.
- A canvas whose view goes while JS draws its frame no longer reports a validation error.
- `GPUExtent3D` and `GPUOrigin3D` truncate fractions (`[canvas.width, canvas.height]` on a fractional pixel ratio), as the spec says.
- `canvas.width` / `height` are whole numbers of pixels, as a `<canvas>`'s and the reference's.
- Windows: an `rgba16float` canvas shows its colors as they are (a float swap chain takes them as linear light: 55 showed as 128).
- Windows: the crate feature `static-dxc` compiles shaders with DXC; the system's FXC refuses some valid WGSL (the gallery's Wireframe).

### Fixed (every platform on screen)

- NSH nightly `589a60f`: on Linux, `requestAnimationFrame` fires (it never did on `f577c51`).
- Linux and Android: concurrent `requestAdapter()` calls no longer panic in wgpu's EGL code.
- Windows, Linux and Android: a device's memory blocks start at 8 MB and grow (wgpu's default takes 256 MB + 64 MB at once).
- Windows: with DX12 the canvas is a composition visual: `alphaMode: "premultiplied"` works and NSH draws over it. The instance there is DX12's alone when it yields an adapter.
- Rust 1.100: no warning in the crate or its manifest.

### Fixed (phase 4a)

- The crate is the repository's root (`Cargo.toml`, `nsh.json`), so `nsh add github:SuperNSH/webgpu` and `nsh add --path <checkout>` read it.
- Leaving a page with an acquired, unpresented frame no longer aborts the macOS/iOS host (the frame ends before its surface is dropped; Android and Windows shared the order).
- Unmounting `<Canvas>` no longer unconfigures its context: it renders offscreen, as the reference's.
- `createRenderPipelineAsync` / `createComputePipelineAsync` compile off the JS thread.
- `createImageBitmap` is declared in the package's types.
