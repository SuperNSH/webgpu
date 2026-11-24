# Parity: surfaces-apple

`<Canvas>`, `GPUCanvasContext`, `requestAnimationFrame` / `cancelAnimationFrame` and the `webgpu.canvas` native view
on macOS and iOS, against react-native-webgpu at 06599f9 (`src/Canvas.tsx`, `src/animationFrame.ts`,
`cpp/rnwgpu/api/GPUCanvasContext.cpp`, `cpp/rnwgpu/SurfaceRegistry.h`, `apple/MetalView.mm`).

## How a canvas reaches wgpu

1. `<Canvas>` (`tsx/Canvas.tsx`) allocates a process-unique context id (`WgpuCanvasContext.allocateContextId`) and
   renders `<View {...props}><WebGPUView contextId … style={{ flex: 1 }} /></View>`, `WebGPUView` being
   `nativeComponent("webgpu.canvas")` with the reference's native props (`contextId`, `appleDisplayP3`,
   `appleExtendedDynamicRange`, `android*`).
2. `#[native_components(macos)]` (`src/surface/macos.rs`) and `#[native_components(ios)]` (`src/surface/ios.rs`)
   register `webgpu.canvas`. The view is an `NSView` hosting a `CAMetalLayer` (layer-hosting) on macOS, a `UIView`
   whose `+layerClass` is `CAMetalLayer` on iOS. On the `contextId` prop (main thread) it tags the layer's
   colorspace (extended sRGB / extended Display P3, EDR for `toneMapping: "extended"`, as `MetalLayerColorSpace.mm`),
   makes `instance().create_surface_unsafe(SurfaceTargetUnsafe::CoreAnimationLayer(layer))`, and
   `views::attach`es it (`src/surface/apple.rs`). Size and backing scale changes (`setFrameSize:`,
   `viewDidChangeBackingProperties`, `layoutSubviews`, `didMoveToWindow`) go to `views::resize`; dropping the
   component calls `canvas_context::view_gone`, which sets the frame in flight aside (it keeps its surface until it ends).
3. `src/api/canvas_context.rs` keeps the swap chain per context id (the reference's `SurfaceInfo`): configure,
   frame, present, an offscreen texture while no surface is attached, and the blit of the last presented offscreen
   frame onto a surface that attaches later (`surface_attached`, called by the view on the main thread).
4. `tsx/api/GPUCanvasContext.ts` holds what only TSX can: the configuration with its device object
   (`getConfiguration`), the current texture's wrapper (the same `GPUTexture` object until the frame ends), and the
   canvas object (`width`/`height` = drawing buffer, `clientWidth`/`clientHeight`, `surface`, no-op DOM methods).

## Rows

| Export | Status | Reason |
|---|---|---|
| `<Canvas>` | implemented | Outer `View` with the app's props, inner native view `flex: 1`, context id per canvas, retired on unmount as `RNWebGPU.destroyContext`: a context object keeps its configuration and renders offscreen once the view went (the reference's `SurfaceInfo`); the native state goes with the view or the last context object, whichever is later. |
| `CanvasProps.colorSpace`, `toneMapping` | implemented (Apple) | Read when the canvas mounts and tagged on the layer on the main thread; a later change warns and is ignored, as the reference. `configure()` warns once when it asks for another colorspace. |
| `CanvasProps.toneMapping` with `rgba16float` and `"standard"` | differs | wgpu's Metal `configure` sets `wantsExtendedDynamicRangeContent` from the format (`rgba16float` → on), overriding the layer tag: values above 1.0 are not clipped. |
| `CanvasProps.android` | passed through | Resolved as the reference (`androidBackingView`, `androidOpaque`, z-order) and sent to the native view; Android's view is surfaces-other's. Ignored on Apple. |
| `CanvasProps.transparent` | kept, no effect on Apple | react-native-wgpu's former prop; on Android it asks for a `TextureView`. Not in the reference at 06599f9. |
| `CanvasProps.ref` | implemented | A callback (react-parity's `useCanvasRef` is a callback with `.current`) or an object with `current`. |
| `CanvasRef.getContext("webgpu")` | implemented | A new context object per call, sharing the canvas's swap chain; the canvas is the view's layout size in points (from `onLayout`, else the native view's), as the reference's `measureLayout`. Another name throws `[WebGPU] Unsupported context: …`. The reference's "Cannot get context before mount" cannot happen: the ref is set when the element exists. |
| `CanvasRef.getNativeSurface()` | implemented | `{ surface: BigInt(CAMetalLayer *), width, height, clientWidth, clientHeight }` in points, plus the no-op DOM methods; `0n` and zeros before the view attached. |
| `CanvasRef.getContextId()` | implemented | |
| `GPUCanvasContext.configure` | implemented | `format` (bgra8unorm, rgba8unorm, rgba16float; else `TypeError`), `usage` (default `RENDER_ATTACHMENT`; bits outside the spec are a validation error), `viewFormats`, `alphaMode`, `colorSpace` / `toneMapping` (unknown values `TypeError` naming `PredefinedColorSpace` / `GPUCanvasToneMappingMode`, as `CanvasContext.spec`). Always `Fifo`, as the reference. |
| `alphaMode: "premultiplied"` on Apple | mapped | wgpu's Metal surfaces offer `Opaque` and `PostMultiplied` only; `PostMultiplied` is a non-opaque `CAMetalLayer`, which Core Animation composites premultiplied: the spec's "premultiplied". |
| `GPUCanvasContext.unconfigure` | implemented | wgpu has no surface unconfigure: the surface stays configured, the context drops its configuration and frame (`getCurrentTexture` then throws `InvalidStateError`). |
| `GPUCanvasContext.getConfiguration` | implemented | Missing in the reference (inventory: "Missing"); here per spec, with defaults filled. |
| `GPUCanvasContext.getCurrentTexture` | implemented | Same object until `present()`, `configure()`, `unconfigure()` or a new `canvas.width`/`height` (which reconfigures); `InvalidStateError` before `configure()`. Without a surface (not attached yet, headless, view gone) an offscreen texture of the same format and size, as the reference. |
| `GPUCanvasContext.present()` | implemented | Presents a surface texture; keeps an offscreen frame, which is shown on the surface when it attaches (copied, or drawn where the surface lacks `COPY_DST`). |
| `canvas.clientWidth` / `clientHeight` | implemented | Updated from the view's size in points at each new texture. |
| `getCurrentTexture().destroy()` within a frame | not verified | The reference allows it (`CanvasContext.spec`); wgpu reports destroying a surface texture as an error. |
| `requestAnimationFrame` / `cancelAnimationFrame` | NSH's | NSH's globals, paced by NSH's frame clock (display link on iOS); the reference uses the runtime's own on macOS too. One timestamp per frame. No worklet runtimes in NSH. |
| `RNWebGPU.MakeWebGPUCanvasContextFromNativeWindow` | not done | No export in this area's files; would be a `create_surface_unsafe` on a caller's `CAMetalLayer *`. |
| `GPUTexture.nativePointer` of a canvas texture | not done | resources' row. |

## Verified

macOS 27.2 (Apple M3 Max), iOS 27.0 simulator `nsh-webgpu-iphone` (iPhone 18 Pro), nightly
`nsh 0.6.0 (589a60f nightly 2026-11-23)`, wgpu 29.0.4. Evidence in `docs/evidence/surfaces-apple/`:

- iOS simulator, on screen: `ios-simulator-screen.png`, `ios-simulator-screen-2.png` (`xcrun simctl io …
  screenshot` while `nsh run --target ios` ran `examples/canvas-apple`; looked at: red Triangle on the `#3498db`
  view showing through the transparent canvas, the reference's colored Cube on `rgb(90,180,255)`, rotated between
  the two pictures, 277 then 643 frames).
- macOS and iOS, the layer's drawable: `macos-triangle-drawable.png`, `macos-cube-drawable.png`,
  `ios-triangle-drawable.png`, `ios-cube-drawable.png` (each canvas's surface texture copied before `present()`,
  logged as `PNG <name>-surface`, from `nsh test --target macos|ios -v canvas.test.tsx`:
  `nsh-test-target-macos-verbose.log`, `nsh-test-target-ios-verbose.log`; looked at). Not a picture of the macOS
  window: `screencapture` fails ("could not create image from window") and `app.screenshot()` returns a black
  picture on this Mac (no Screen Recording permission; a plain NSH view gives the same black picture), so on-screen
  compositing on macOS is unverified by picture; the status `surface attached` and 60 fps cube frames are.
- `cargo test --workspace` (`cargo-test.txt`: 11 + 4 tests; `tests/surfaces_apple` includes a real `CAMetalLayer`
  surface: drawable size follows the canvas, a frame drawn and read back from the drawable, the view-gone
  lifecycle), clippy (`cargo-clippy.txt`, `cargo-clippy-ios-sim.txt`), fmt (`cargo-fmt.txt`).
- `nsh check` / `nsh build` / `nsh test` in `examples/headless` (19 tests, `surfaces-apple.test.tsx` 3 of them: the
  context through a mounted `<Canvas>`, the Triangle read back from the canvas texture, rAF) and in
  `examples/canvas-apple`.
- Seen failing on purpose: `broken-on-purpose-cargo-test.txt` (no frame reuse), `broken-on-purpose-nsh-test-*.txt`
  (frame reuse, `getConfiguration`, rAF).

Not verified: a picture of the macOS window; Mac Catalyst; a physical iPhone; `colorSpace: "display-p3"` and
`toneMapping: "extended"` visually; resizing a window while rendering (the code path is tested with a layer only).
