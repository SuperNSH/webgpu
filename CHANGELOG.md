# Changelog

## 0.1.0 (unreleased)

The first version: react-native-webgpu (`06599f9`) ported to NSH on wgpu 29, for the NSH nightly `f577c51`.

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
- webgpu-samples' licence text is still to be pasted into `NOTICE.md`, and the Stanford dragon mesh's terms are not established (`NOTICE.md`).
- Apps need a Rust crate of their own pinned to `f577c51` (README, "Status").

### Fixed (phase 4c, after the audit in `docs/audit/REPORT.md`)

- `NOTICE.md` and `examples/gallery/LICENSES.md`: react-native-webgpu's MIT notices, @webgpu/types, webgpu-samples, wgpu-matrix, the gallery's assets.
- `device.destroy()` releases the device's id and listeners; a device the app drops is collected and released (weak listeners, a `FinalizationRegistry`).
- `<Canvas>` sets its ref once mounted and laid out, sets it to `null` on unmount, and `getContext()` on an unmounted canvas throws `[WebGPU] Cannot get context before mount`, as the reference's; a destroyed context id is never made again. `children` are accepted (not rendered), `NativeProps` is exported, and the change warnings are development-only, with the reference's Android ones.
- `useSurface()` follows the canvas's mount, layout, surface attach and unmount.
- The reference's `declare global` augmentations of the WebGPU types.
- `createTestVideoFrame(width, height)` has the reference export's signature.
- Overflow-checked buffer ranges, the Android window released after its surface is replaced, a replaced surface dropped outside the views lock, `Wgpu.release` guarded, a ticket dropped when its wait gives up; Cargo metadata.

### Fixed (phase 4a)

- The crate is the repository's root (`Cargo.toml`, `nsh.json`), so `nsh add github:SuperNSH/webgpu` and `nsh add --path <checkout>` read it.
- Leaving a page with an acquired, unpresented frame no longer aborts the macOS/iOS host (the frame ends before its surface is dropped; Android and Windows shared the order).
- Unmounting `<Canvas>` no longer unconfigures its context: it renders offscreen, as the reference's.
- `createRenderPipelineAsync` / `createComputePipelineAsync` compile off the JS thread.
- `createImageBitmap` is declared in the package's types.
- Windows and Linux are compile-checked only.
