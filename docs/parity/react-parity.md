# Parity: react-native-webgpu's React API and extensions (area react-parity)

The exports of inventory table 1 that this area owns, against the reference (react-native-webgpu `06599f9`,
`packages/webgpu/src/{hooks.tsx,GPUDeviceProvider.tsx,Offscreen.ts,formats.ts,video.ts,importDevice.ts,NativeBufferView.tsx,NativeWebGPUModule.ts,types.ts}`).
Files: `tsx/{hooks.tsx,GPUDeviceProvider.tsx,Offscreen.ts,formats.ts,video.ts,importDevice.ts,NativeBufferView.tsx,WebGPUModule.ts,types.ts}`.
Tests: `examples/headless/react-parity.test.tsx`, which opens `examples/headless/react-parity.app.tsx`.

> Area notes from phase 2. The status of every export is in [docs/parity.md](../parity.md), which is current;
> rows here that phase 3 (integration) changed are updated and marked "phase 3".

## React on NSH

NSH TSX has signals and JSX, no React. A component runs once; only the bindings that read a changed signal run
again. Each hook keeps the reference's name, arguments and return shape; what React would re-render with is a
signal underneath, and the returned objects' members are **getters reading signals**. Read them in JSX or an
`effect` (`state.device`), and they update. Destructuring in a component body (`const { device } = useDevice()`)
takes a snapshot, as reading any signal there does; inside `<GPUDeviceProvider>`, `useMainDevice()` is safe to
destructure because the children only mount once the device exists, as in the reference.

| Reference (React) | NSH mapping |
|---|---|
| `useDevice(adapterOptions?, deviceDescriptor?)` → `{ adapter, device }` | Starts `requestAdapter` → `warnIfNotHardwareAccelerated` → `requestDevice` when called; one `signal` holds the pair; returns `{ get adapter(), get device() }`, `null` until ready. Same errors (`No appropriate GPUAdapter found.`, `No appropriate GPUDevice found.`) thrown in the request's promise, as the reference's async IIFE does. Never destroys the device (nor does the reference). |
| `useCanvasRef()` → `useRef<CanvasRef>(null)` | A function `(canvas) => void` (the callback `<Canvas ref>` takes in NSH) with a `current` property backed by a signal: `<Canvas ref={ref}>`, then `ref.current`. Type `CanvasRefObject`. |
| `useSurface()` → `{ ref, surface }` (`useLayoutEffect` reads `getNativeSurface()`) | `ref` is a `useCanvasRef()`; `surface` is a getter on a `computed` that calls `getNativeSurface()` when the canvas mounted, and again when it lays out anew or its surface attaches (the canvas's internal surface signal), `null` before mount and after unmount. The reference crashes when no canvas is rendered; here `surface` stays `null`. |
| `warnIfNotHardwareAccelerated(adapter)` | Same code: `console.warn` with the reference's text when `adapter.info.isFallbackAdapter` (here: `forceFallbackAdapter` was asked for, or wgpu reports device type `Cpu`; `src/api/adapter.rs`). |
| `GPUDeviceProvider` (`createContext` + `useDevice`, renders `null` until ready) | An NSH `context()`; the provider calls `useDevice` and renders `<If when={state.device}>` around `<DeviceContext.Provider>`; `props.children` is read inside the provider, so children mount once the device exists. Props `DeviceProviderProps` (`children`, `adapterOptions`, `deviceDescriptor`), as the reference. |
| `useMainDevice()` (`useContext`) | Reads the context; throws `No DeviceContext found.` outside a provider. |
| `useNativeBufferSize(debounceMs = 150)` → `{ size, onPixelSizeChange }` | `useState` → a signal behind a `size` getter; `useRef` → closure variables; `setTimeout`/`clearTimeout` as in the reference; the unmount effect → NSH `cleanup()`. Behaviour identical (first size at once, later after `debounceMs` of stability, `<= 0` at once). |
| `WebGPUModule` (TurboModule `{ install(): boolean }`) | `{ install() }` calls `installWebGPU()` (idempotent) and returns `true`; the package import already installs WebGPU, as the reference's entry does. |
| `formats.ts` `Platform.OS` | `platform.os` from `nsh`. |

## Coverage

| Export | Status | Notes / reason |
|---|---|---|
| `useDevice` | implemented | mapping above |
| `useCanvasRef` | implemented | mapping above; works with any component calling its `ref` prop (area surfaces' `<Canvas>` takes `ref?: (canvas: CanvasRef) => void`) |
| `useSurface` | implemented | mapping above |
| `warnIfNotHardwareAccelerated` | implemented | same text and condition |
| `GPUDeviceProvider`, `useMainDevice` | implemented | the stub's extra `loading` prop was dropped: the reference has none (its docs: render a loading view next to the provider) |
| `GPUOffscreenCanvas` (`width`, `height`, `getContext("webgpu")`, `getImageData()`) | implemented | context not exported, as in the reference; `configure` ignores `config.format` and uses `getPreferredCanvasFormat()`, `getConfiguration()` throws `Method not implemented.`, `present()` no-op, `unconfigure()` destroys the texture, as the reference. **Difference**: `getImageData()` copies rows padded to 256 bytes and drops the padding, so any width works; the reference copies `width * 4`-byte rows, valid only for widths that are multiples of 64 (its docs say so). Same result for those widths. |
| `getPreferredHighBitDepthCanvasFormat` | implemented | `"rgb10a2unorm"` on Android, `"rgba16float"` elsewhere |
| `installWebGPU` | implemented (orchestrator's `install.ts`) | |
| `WebGPUModule` | implemented | mapping above |
| `createTestVideoFrame(width, height)` | implemented, in memory | The reference export's signature. The reference's patterns byte for byte (bgra8: R = x·255/(w−1), G = y·255/(h−1), B = 220 if (x+y)&0x20 else 30, A = 255, BGRA order; nv12: BT.709 limited range, Y 63/173, CbCr 102,240 / 42,26, red left half, green right). The frame lives in memory, not an IOSurface: `handle` is `0n`, `acquireFenceFd` `null`. Extension: `plane(i)` gives the bytes (bgra8: plane 0; nv12: 0 = Y, 1 = CbCr), for `queue.writeTexture`. nv12 is what the reference's `RNWebGPU.createTestVideoFrame` global makes with a third argument; here it is the internal `testVideoFrame` (`tsx/video.ts`), not exported. Width/height outside 1..16384 or non-integer throw `RangeError`. |
| `createVideoPlayer(source, options?)` | `ERR_WEBGPU_UNSUPPORTED` | needs a platform decoder (AVPlayer, MediaCodec) and zero-copy import of its frames; wgpu has no shared texture memory |
| `createNativeBufferFromPointer(pointer)` | `ERR_WEBGPU_UNSUPPORTED` | wraps a CVPixelBufferRef / AHardwareBuffer*; wgpu cannot import it |
| `createNativeBuffer(width, height, options?)` | `ERR_WEBGPU_UNSUPPORTED` | an IOSurface / AHardwareBuffer to render into needs shared texture memory, which wgpu lacks |
| `NativeBufferView` | `ERR_WEBGPU_UNSUPPORTED` (thrown when rendered) | no NSH native view displays a platform buffer, and no buffers can be made (above). Its prop and ref types (`NativeBufferViewProps`, `NativeBufferViewRef` with `viewId` and `present(frame, done?)`, `NativeBufferResizeMode` incl. `topLeft`, `NativeBufferPixelSize`) match the reference. |
| `useNativeBufferSize` | implemented | pure logic, works with any producer of pixel sizes |
| `importDevice(pointer)` | `ERR_WEBGPU_UNSUPPORTED` | wraps a Dawn `WGPUDevice*` (react-native-skia Graphite); wgpu objects are not Dawn's |
| `adoptTexture(pointer)` | `ERR_WEBGPU_UNSUPPORTED` | adopts a Dawn `WGPUTexture*`; same reason |
| Types (`types.ts`) | match the reference | `NativeCanvas` (with the no-op DOM stubs), `RNCanvasContext`, `CanvasRef`, `NativeBuffer` (`handle`, `acquireFenceFd`), `NativeBufferPixelFormat`, `NativeBufferColorSpace`, `GPUExtent2D`, `GPUNativeExternalTextureDescriptor`, `VideoPlayer`, `GPUCopyExternalNativeFrameSourceInfo`, `CreateVideoPlayerOptions`, `GPUDrawElementImageSourceView` (`React.Component` → `object`), `GPUDrawElementImageSource`, `GPUDrawElementImageDestination`, `GPUSharedTextureMemoryDescriptor`, `GPUSharedFenceType`, `GPUSharedFenceDescriptor`, `GPUSharedFenceExportInfo`, `GPUSharedFence`, `GPUSharedFenceState`, `GPUSharedTextureMemoryEndAccessState`, `GPUDawnTogglesDescriptor`, `GPUSharedTextureMemory`, `CreateNativeBufferOptions`. The stub's wrong shapes (`NativeBuffer.pointer`, `VideoPlayer.nextFrame/seek`, `CreateVideoPlayerOptions.source/loop`) were replaced. |

## Evidence (macOS, Metal, nightly `nsh 0.6.0 (589a60f nightly 2026-11-23)`)

| Claim | Command (in `examples/headless`, `NSH_HOME=$PWD/../../.nsh-home`) | Output |
|---|---|---|
| Types check | `nsh check` | `docs/evidence/react-parity/nsh-check.txt` |
| Builds with no warning | `nsh build` | `docs/evidence/react-parity/nsh-build.txt` |
| All TSX tests pass (react-parity: 11 passed, 1 skipped) | `nsh test` | `docs/evidence/react-parity/nsh-test.txt` |
| Every react-parity test fails when its code is broken on purpose (11 failed) | `nsh test react-parity.test.tsx` with 11 deliberate breaks, then restored | `docs/evidence/react-parity/nsh-test-broken-on-purpose.txt` |
| Cargo gates (untouched Rust) | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` | `docs/evidence/react-parity/cargo-{fmt,clippy,test}.txt` |

Phase 3: "a frame of `createTestVideoFrame` drawn into a `GPUOffscreenCanvas` reads back byte for byte" runs (no
longer `test.skip`). The canvas's texture is `RENDER_ATTACHMENT | COPY_SRC | TEXTURE_BINDING` whatever `configure()`
asks, as the reference's `Offscreen.ts` makes it, so a `writeTexture` into it is a validation error (no `COPY_DST`);
the app now uploads the frame to a texture of its own and draws it pixel for pixel onto the canvas, as an app on the
reference must (`docs/evidence/integration/broken-on-purpose-offscreen-frame-copy.txt` shows the copy failing).
`useCanvasRef`/`useSurface` were first tested with a stand-in canvas; since phase 4c they are tested with the real
`<Canvas>` (`react-parity.app.tsx` scenario `device`, `canvas-lifecycle.test.tsx`). Only macOS headless was run; the Android branch of `getPreferredHighBitDepthCanvasFormat` is unverified.
