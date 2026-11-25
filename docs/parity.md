# Parity with react-native-webgpu

Every export of react-native-webgpu (commit `06599f9`, `packages/webgpu/src`; the list is `docs/inventory.md` §1–2)
and what nsh-webgpu does with it, on wgpu 29.0.4 and NSH 0.7.1. One table per area. Status:

- **done**: the reference's name, signature, defaults and errors.
- **partial**: works, with the difference given.
- **unsupported**: the name exists and throws (or rejects with) an `Error` whose `code` is `ERR_WEBGPU_UNSUPPORTED`,
  for the reason given.
- **n/a**: React Native machinery with no NSH meaning; what replaces it is given.

The area files hold the design, the tests ported from `packages/webgpu/src/__tests__` and the evidence:
[resources](parity/resources.md), [commands](parity/commands.md), [surfaces-apple](parity/surfaces-apple.md),
[surfaces-other](parity/surfaces-other.md), [react-parity](parity/react-parity.md). Where an area file and this
page disagree, this page is current (phase 3, `docs/evidence/integration/`).

Verified on macOS 27.2 (Apple M3 Max, Metal), the iOS 27 simulator, an Android emulator (GL) a browser, and on screen in CI on macOS, Windows (DX12 on WARP) and
Linux (Vulkan on lavapipe) (the platform table in `docs/architecture.md`).

## `navigator.gpu`, adapter, device (foundation)

| Export | Status | Notes / reason |
|---|---|---|
| `navigator.gpu` (`GPU`): `requestAdapter`, `getPreferredCanvasFormat`, `wgslLanguageFeatures` | done | installed on import by `installWebGPU()`; on the web the browser's own stays (below) |
| `requestAdapter({ featureLevel })` | partial | accepted, ignored: wgpu has one level |
| `requestAdapter({ powerPreference, forceFallbackAdapter })` | done | |
| `GPUAdapter`: `info`, `features`, `limits`, `requestDevice` | done | a second `requestDevice` rejects (consumed adapter) |
| `GPUAdapterInfo`: `vendor`, `device`, `description`, `subgroupMinSize`, `subgroupMaxSize`, `isFallbackAdapter`, `toJSON` | done | `vendor` from the PCI id (`"apple"` for Apple's Metal adapters); `isFallbackAdapter`: `forceFallbackAdapter` asked, or wgpu device type `Cpu` |
| `GPUAdapterInfo.architecture` | partial | always `""`: wgpu reports no GPU family |
| `GPUDeviceDescriptor.dawnToggles`, `implicitDeviceSynchronization` | partial | accepted, ignored: Dawn-only (a wgpu device is `Send + Sync`) |
| `GPUDevice`: `features`, `limits`, `adapterInfo`, `queue`, `label`, `destroy`, `lost` | done | `lost` settles on `destroy()` at once, else from wgpu's lost callback. `destroy()` then releases the device's id and its TSX listeners; its objects still find the destroyed device (no exceptions, as the spec's lost device), and the handle goes when the `GPUDevice` is collected. A device the app drops undestroyed is collected (weak listeners) and released by a `FinalizationRegistry` (`examples/headless/webgpu.test.tsx`) |
| `GPUDevice.pushErrorScope` / `popErrorScope` | done | per device, kept by nsh-webgpu (wgpu's scopes are per thread) |
| `uncapturederror` (`addEventListener`, `onuncapturederror`), `GPUUncapturedErrorEvent` | done | warns on the console when nobody listens, as browsers do |
| `GPUDevice.forceLossForTesting` | unsupported | Dawn's testing hook; wgpu cannot lose a device on request |
| `GPUError`, `GPUValidationError`, `GPUOutOfMemoryError`, `GPUInternalError`, `GPUPipelineError`, `GPUDeviceLostInfo` | done | `instanceof` hierarchy as the reference (Constants.spec) |
| `GPUBufferUsage`, `GPUTextureUsage`, `GPUShaderStage`, `GPUColorWrite`, `GPUMapMode` | done | |
| `installWebGPU` | done | puts `navigator.gpu`, the classes and the constants on `globalThis`, idempotent; on the web only `createImageBitmap` learns encoded bytes, as the reference's web entry (the browser's WebGPU stays) |
| `WebGPUModule` (`install()`) | done | calls `installWebGPU()`, returns `true` |
| `__brand` on every class | done | |
| `requestAnimationFrame` / `cancelAnimationFrame` | done | NSH's globals, paced by NSH's frame clock |

## Resources (area a)

| Export | Status | Notes / reason |
|---|---|---|
| `device.createBuffer`, `GPUBuffer` (`mapAsync`, `getMappedRange`, `unmap`, `destroy`, `size`, `usage`, `mapState`) | partial | all members and errors; `getMappedRange` returns a copy written back on `unmap()` (wgpu's mapped memory stays in Rust; `docs/architecture.md` item 6) |
| `device.createTexture`, `GPUTexture` (getters, `createView`, `destroy`) | done | all 101 spec formats; wgpu-only formats → `TypeError` |
| `GPUTexture.nativePointer` | unsupported | a Dawn `WGPUTexture*`; wgpu has no such handle |
| `GPUTexture.textureBindingViewDimension` | partial | reported, ignored (compatibility mode; a wgpu device is a core device) |
| `GPUTextureView` | done | every `GPUTextureViewDescriptor` member |
| `device.createSampler`, `GPUSampler` | partial | `maxAnisotropy` clamped to 16 (wgpu's maximum); wgpu-only address modes → `TypeError` |
| `device.createBindGroupLayout`, `GPUBindGroupLayout` | done | buffer, sampler, texture, storageTexture, externalTexture entries (externalTexture needs wgpu's `EXTERNAL_TEXTURE`, which `requestDevice` enables when the adapter has it) |
| `device.createBindGroup`, `GPUBindGroup` | done | samplers, views, textures, buffers, buffer bindings and `GPUExternalTexture` (phase 3: `Bound::External`) |
| `device.createPipelineLayout` | done | null slots kept |
| `device.createShaderModule`, `getCompilationInfo`, `GPUCompilationInfo`, `GPUCompilationMessage` | done | WGSL; messages with every spec field (the reference installs none); `compilationHints`/`sourceMap` accepted, ignored |
| `createRenderPipeline(Async)`, `GPURenderPipeline.getBindGroupLayout` | partial | see the vertex-buffer hole below |
| `createComputePipeline(Async)`, `GPUComputePipeline.getBindGroupLayout` | done | |
| `device.createQuerySet`, `GPUQuerySet` | partial | `destroy()` marks the set (wgpu's `QuerySet` has no `destroy`); every pass checks it |
| `GPUSupportedFeatures`, `WGSLLanguageFeatures` | partial | WebGPU names only; `texture-formats-tier2`, `texture-component-swizzle`, `subgroup-size-control`, `texture-compression-unaligned`, `atomic-vec2u-min-max` never reported (wgpu 29 has none) |
| `GPUSupportedLimits` | partial | all 36 limits; `maxBindGroupsPlusVertexBuffers` = `maxBindGroups + maxVertexBuffers`; the per-stage storage limits = the per-shader-stage ones |

## Commands (area b)

| Export | Status | Notes / reason |
|---|---|---|
| `GPUCommandEncoder` (`beginRenderPass`, `beginComputePass`, copies, `clearBuffer`, `resolveQuerySet`, debug groups, `finish`) | done | passes record and replay at `end()`; errors at `finish()` as the spec |
| `GPURenderPassEncoder` (all members) | partial | `maxDrawCount` accepted, ignored (wgpu has no draw-count limit) |
| `GPUComputePassEncoder` (all members) | done | |
| `GPURenderBundleEncoder`, `GPURenderBundle` | partial | debug groups and markers accepted, not recorded (wgpu's bundle encoder has none) |
| `GPUCommandBuffer` | done | |
| `GPUQueue.submit`, `writeBuffer`, `writeTexture`, `onSubmittedWorkDone` | done | |
| `GPUQueue.copyExternalImageToTexture` (`ImageBitmap`, `ImageData`) | done | bit-exact alpha conversion, every spec destination format |
| `copyExternalImageToTexture` / `importExternalTexture` with a `NativeBuffer`, video, canvas or image element | unsupported | NSH has no DOM; wgpu cannot import platform buffers without HAL interop |
| `GPUQueue.drawElementImageToTexture` | unsupported | NSH has no hook that draws a view subtree into pixels |
| `device.importExternalTexture` (`GPUTexture` source), `GPUExternalTexture` (`destroy`) | partial | one RGBA plane through wgpu 29's `ExternalTexture`; crop, `mirrored`, `rotation`, `colorSpace: "display-p3"` applied; binds in a bind group and renders (phase 3). Without wgpu's `EXTERNAL_TEXTURE` on the device: unsupported. Metal and DX12 only: wgpu 29 has no `EXTERNAL_TEXTURE` on Vulkan or GL, so on Linux and Android it throws `ERR_WEBGPU_UNSUPPORTED` |
| `createImageBitmap`, `ImageBitmap` | done | PNG/JPEG/WebP bytes, `ImageData`, `ImageBitmap`, Blob-like; crop, resize and `flipY`, which the reference's native decoder ignores; declared globally with the bytes overloads (`tsx/types/dom.d.ts`) |
| `createImageBitmap` `colorSpaceConversion`, `imageOrientation: "from-image"` | partial | accepted; no ICC, no EXIF orientation |
| A `null` hole in `vertex.buffers` before a used slot | partial (wrong slot) | wgpu-core 29 bug, fixed in wgpu 30 (#9351): repro and draft in `docs/upstream/wgpu-null-vertex-slot/` |

## Canvas and surfaces (areas c and d)

| Export | Status | Notes / reason |
|---|---|---|
| `<Canvas>` (`CanvasProps`: view props, `ref`, `colorSpace`, `toneMapping`, `children`) | done | outer `View` + native `webgpu.canvas` (`flex: 1`); on the web a `<canvas>` in the view with the browser's WebGPU, as the reference's web entry. `children` are accepted and not rendered, as the reference's native view replaces them. The ref is set once the canvas is mounted (its view's first layout, so `getContext()` sizes to a real layout) and set to `null` on unmount; `getContext()` on an unmounted canvas throws `[WebGPU] Cannot get context before mount` (`examples/headless/canvas-lifecycle.test.tsx`). Change warnings are development-only (`appInfo().development`) and include the reference's Android ones |
| `CanvasProps.android` (`backingView`, `opaque`, `zOrderOnTop`, `zOrderMediaOverlay`) | partial | resolved as the reference's `androidNativeProps`; `HardwareBufferView` falls back to a `TextureView` with a warning (needs the reference's Java `HardwareRenderer` path; an nshm ships no Java) |
| `AndroidBackingView`, `AndroidCanvasProps` and its three members | done | |
| `CanvasProps.transparent` | partial | react-native-wgpu's former prop, kept: a `TextureView` on Android, no effect elsewhere |
| `CanvasRef.getContextId`, `getNativeSurface`, `getContext("webgpu")` | done | `getNativeSurface().surface`: the `CAMetalLayer`/`ANativeWindow` pointer, `0n` on Linux (no native surface), the context id on the web; its size is the layout's until the view reports one. A retired canvas's native state is never made again (`WgpuCanvasContext.make` refuses a destroyed id) |
| `GPUCanvasContext.configure`, `unconfigure`, `getCurrentTexture`, `present`, `canvas` | done | Fifo always, as the reference; with no surface the frame renders offscreen and shows when one attaches |
| `GPUCanvasContext.getConfiguration` | done | missing in the reference; here per spec |
| `alphaMode: "premultiplied"` on Apple | partial | wgpu's Metal surfaces offer `PostMultiplied`, a non-opaque `CAMetalLayer` that Core Animation composites premultiplied |
| `alphaMode` a surface cannot do (Android's GL surfaces offer `Opaque` only) | partial | the surface's own mode instead (opaque when offered), as the reference honours `alphaMode` on Apple only (phase 3; before, `configure` failed and nothing showed on Android) |
| `configure` with a usage, view formats or a format the surface lacks (Android GL: render attachment only, so no `COPY_SRC`) | done | the canvas renders offscreen and is drawn onto the surface at `present()` |
| `configure({ format: "rgb10a2unorm" })` on Android | done | not the spec's; what `getPreferredHighBitDepthCanvasFormat()` names there, as the reference |
| `colorSpace: "display-p3"`, `toneMapping: "extended"` | partial | tagged on the Apple layer at mount; `rgba16float` with `"standard"` is not clipped (wgpu sets EDR from the format); Android ignores them, as the reference |
| `getCurrentTexture().destroy()` within a frame | partial (not verified) | wgpu reports destroying a surface texture as an error |
| `RNWebGPU.MakeWebGPUCanvasContextFromNativeWindow` | unsupported (not exported) | NSH has no `RNWebGPU` global; would be `create_surface_unsafe` on a caller's layer/window |
| `GPUOffscreenCanvas` (`width`, `height`, `getContext`, `getImageData`) | partial | as the reference (one texture, `format` ignored, `getConfiguration` throws, `present` no-op), except that `getImageData` reads any width (rows padded to 256), where the reference's needs a multiple of 64 |

| Platform | Canvas | Status |
|---|---|---|
| macOS | `NSView` hosting a `CAMetalLayer` | run on screen (CI) |
| iOS | `UIView` whose layer is a `CAMetalLayer` | run on screen (simulator) |
| Android | `FrameLayout` holding a `TextureView` or `SurfaceView`, `ANativeWindow` → surface | run on screen (emulator, GL) after phase 3: the view itself (`examples/canvas-native`) and `<Canvas>` + `GPUCanvasContext` (`examples/canvas-apple`, the rAF Cube) |
| Windows | a DirectComposition visual (DX12); a child `HWND`, opaque and on top, without a DX12 adapter or with `WGPU_BACKEND` set | run on screen (CI, DX12 on WARP); the child window was too |
| Linux | `gtk::Picture` fed by offscreen frames read back at each `present()` | run on screen (CI and a container, Vulkan on lavapipe) |
| Web | `<canvas>` in the view, the browser's WebGPU | run on screen (browser; an app using the package, `docs/evidence/platforms/web-readme-app.jpg`) |
| wasm build of the crate | wgpu's WebGPU backend | `cargo clippy --target wasm32-unknown-unknown` passes; linked into the app's web runtime, where the browser's own WebGPU is used instead |

## React API and extensions (area e)

| Export | Status | Notes / reason |
|---|---|---|
| `useDevice`, `useCanvasRef`, `useSurface`, `warnIfNotHardwareAccelerated` | done | signals behind getters (read them in JSX or an effect); `useCanvasRef` is a callback ref with `.current`; `useSurface().surface` follows the canvas's mount, layout, surface attach and unmount (tested with the real `<Canvas>`, `examples/headless/canvas-lifecycle.test.tsx`) |
| `GPUDeviceProvider`, `useMainDevice`, `DeviceProviderProps` | done | |
| `useNativeBufferSize` | done | |
| `getPreferredHighBitDepthCanvasFormat` | done | `rgb10a2unorm` on Android, `rgba16float` elsewhere |
| `createTestVideoFrame(width, height)` | partial | the reference export's signature; its bgra8 pattern, in memory (`handle` is `0n`); `plane(i)` (not on the type) gives the bytes. The nv12 frame of the reference's `RNWebGPU` global is the internal `testVideoFrame` |
| `createVideoPlayer`, `createNativeBuffer`, `createNativeBufferFromPointer` | unsupported | need platform decoders and buffers wgpu cannot import (no shared texture memory) |
| `NativeBufferView` (and its prop/ref types) | unsupported | throws when rendered: no NSH view shows a platform buffer, and none can be made |
| `device.importSharedTextureMemory`, `importSharedFence`, `GPUSharedTextureMemory`, `GPUSharedFence` | unsupported | wgpu has no shared texture memory or fences (the types are kept) |
| `importDevice`, `adoptTexture` | unsupported | wrap Dawn `WGPUDevice*` / `WGPUTexture*`; wgpu objects are not Dawn's |
| `types.ts` (`NativeCanvas`, `RNCanvasContext`, `CanvasRef`, `NativeBuffer`, video, shared memory, fence, draw-element and Dawn toggle types) | done | same names and members as the reference |
| `NativeProps` (the native view's props, `WebGPUViewNativeComponent`) | done | exported from `tsx/Canvas.tsx` |
| the `declare global` augmentations (`GPUTexture.nativePointer`, `GPUDevice.importExternalTexture`/`importSharedTextureMemory`/`importSharedFence`, `GPUQueue.drawElementImageToTexture`/`copyExternalImageToTexture` with a native frame, `GPUDeviceDescriptor.dawnToggles`/`implicitDeviceSynchronization`, `GPUExternalTextureDescriptor.rotation`/`mirrored`, `GPUExternalTexture.destroy`, `createImageBitmap` on bytes) | done (types) | in `tsx/index.tsx` (`createImageBitmap` in `tsx/types/dom.d.ts`), each with a note on what runs: the unsupported ones throw `ERR_WEBGPU_UNSUPPORTED`. The `RNWebGPU` and `__webgpuAnimationFrames` globals are not declared: nsh-webgpu installs neither |

## React Native machinery (n/a)

| Reference | What NSH has instead |
|---|---|
| the `RNWebGPU` global (`allocateContextId`, `getNativeSurface`, `MakeWebGPUCanvasContext`, `destroyContext`, `DecodeToUTF8`, `fabric`, video and buffer factories, `NativeBufferView` registry) | the native module `nsh-webgpu/native` (`WgpuCanvasContext`, ...), used by `<Canvas>` and the exports above; `DecodeToUTF8`: NSH's `TextDecoder` |
| `WebGPUViewNativeComponent` (codegen view) | the native component `webgpu.canvas`, registered per platform with `#[native_components]` |
| worklets: `"worklet"` functions, `registerWebGPUForReanimated`, `__webgpuWorkletHelpers` | none: NSH has no worklet runtimes |
| `__webgpuAnimationFrames` | NSH's frame clock |
| `mock.ts` (Jest) | `nsh test` runs the real module headless on the GPU |
| Dawn blob cache, Vulkan pipeline cache, Hermes memory pressure, iOS background submit hold | none (Dawn, Hermes and RN lifecycle specific) |

## NSH blockers found

Listed with the evidence in `docs/architecture.md` "Blockers and limits in NSH"; in short: `nsh add --path` cannot read `version.workspace`; `#[module]` needs the literal
`Result`; the web player's realm is an iframe (`parent.document`); no Java in an nshm (Android surface lifecycle by
`AChoreographer`, no `HardwareBufferView`); GTK 4 widgets have no native surface (Linux reads frames back).

## Known limitations

Found by the audit (`docs/audit/REPORT.md`) and left as they are, with the reason:

| | Limitation | Why it stays |
|---|---|---|
| N2 | An error made off the JS thread (a blocking poll in `Wgpu.wait`) lands in the device's innermost open error scope at that moment, which may be a scope the JS thread pushed after the call that caused it. | The error scopes are kept per device (wgpu's are per thread), and wgpu reports such an error without saying which call made it. In practice the work that waits (`mapAsync`, `onSubmittedWorkDone`) reports its own errors on the JS thread first. |
| N3 | The UI thread's attach (`surface_attached`) and the JS thread's first `getCurrentTexture()` may both configure the surface. | Each holds the context lock while it configures, so the two configurations are whole and the second wins with the same values; the cost is one redundant `configure`. |
| N5 | On wasm, `requestAdapter`/`requestDevice` run their wgpu future through `pollster::block_on` in place, which would never finish on the browser's event loop. | No app calls them on wasm: on the web, nsh-webgpu uses the browser's own WebGPU from TSX (`tsx/surface/web.tsx`). |
| N8 | Linux reads each presented frame back to the CPU on an OS thread of its own. | Linux shows offscreen frames in a `gtk::Picture` (GTK 4 gives a widget no native surface); a thread pool would save the threads. |
| | `Wgpu.wait` gives up after 8 polls of an unfinished ticket; its ticket is dropped then (N7, fixed), and the `mapAsync` it served rejects with an `OperationError`. | |
| | The `RNWebGPU` and `__webgpuAnimationFrames` globals are not installed or declared. | Their functions are the package's exports and NSH's frame clock. |
| | A destroyed device's wgpu handle stays (out of the registry, its GPU memory freed by `destroy()`) until its `GPUDevice` is collected. | Its objects (queues, buffers, encoders) find their device by id and must keep behaving as a lost device's objects do, without exceptions. |
