# Known issues

What the gallery could not port faithfully. The demos stay in the list either way, and the tests
(`gallery.test.tsx`) assert what they do.

## Crate bugs (nsh-webgpu): fixed

The gallery found five bugs in the crate; all are fixed, and the tests that asserted them now assert the correct
behaviour (`docs/evidence/fixes/gallery-flipped-tests.txt`, each seen failing with the bug put back).

| Was | Fix | Test |
|---|---|---|
| `createBindGroup` refused a `GPUExternalTexture` (Import External Texture threw `object <id> is not a GPUBindingResource`) | `Bound::of` takes the external texture (`src/api/bind_group.rs`) | `ImportExternalTexture` draws (1145 colors, no GPU error) |
| Leaving a page while a frame was acquired and not presented aborted the macOS and iOS hosts: the view's teardown dropped the `wgpu::Surface` before the frame's `SurfaceTexture`, and wgpu-core panicked (`Surface[..] does not exist`) inside `dealloc` (`docs/evidence/gallery/crash-*.txt`) | `canvas_context::view_gone` ends the frame, then drops the surface, and catches any panic; a surface frame holds its surface, so the texture always goes first. Android and Windows had the same order and call the same function now | `tests/surfaces_apple` `a_view_going_with_an_unpresented_frame_does_not_panic`, on a real `CAMetalLayer` (`docs/evidence/fixes/crash-teardown-test.txt`) |
| Unmounting `<Canvas>` unconfigured its context (Render After Unmount: `InvalidStateError ... not configured` every frame) | as the reference's `SurfaceInfo`: a context object keeps its state after the canvas unmounts and renders offscreen once the view went; the native state goes with the last context object (a `FinalizationRegistry`) | `RenderAfterUnmount` renders frames 120 and 240 after the unmount, none throws |
| `createRenderPipelineAsync` / `createComputePipelineAsync` compiled on the JS thread (Async Runner Starvation: "blocked") | read on the JS thread, compiled on a blocking thread in wgpu's own (per-thread) error scopes; an invalid pipeline still rejects with a `GPUPipelineError` | `examples/headless/webgpu.test.tsx` "create*PipelineAsync compile off the JS thread" (`docs/evidence/fixes/async-pipelines-test.txt`) |
| `createImageBitmap` had no TypeScript declaration | declared in the crate's `tsx/types/dom.d.ts`; the gallery's own `src/globals.d.ts` is gone | `nsh check` in this directory |

Async Runner Starvation still says "blocked" when the driver has the pipeline cached: it is ready in a millisecond
or two, before any timer or frame is due. The test accepts that only for a time under 10 ms.

## Not as in the reference

| Demo | Difference | Why |
|---|---|---|
| A-Buffer, Wireframe | the Stanford dragon (centered, scaled to the teapot's extent) where the reference draws the Utah teapot | the reference imports the npm package `teapot`; NSH's bundler resolves no npm packages, and the data (public-domain geometry) is not on this machine to vendor: the reference checkout has no `node_modules`, and no npm, Yarn or Bun cache holds the package (searched in phase 4a); fetching it needs the user's go-ahead. `src/components/meshes/teapot.ts` is the stand-in; the reference's `abuffer.png` snapshot is therefore not compared |
| Compute Boids, Gradient Tiles, MNIST Inference | TypeGPU's TypeScript-to-WGSL functions written in WGSL by hand, its buffers and bind groups as plain WebGPU | TypeGPU (`typegpu`, `@typegpu/react`) is an npm package |
| MNIST Inference | the drawing pad is a WebGPU canvas fed by touch events, the stroke rasterized in TypeScript (1 cell wide, anti-aliased) | react-native-skia, gesture-handler, reanimated and worklets are React Native libraries |
| Resize | the width animates with a signal driven by requestAnimationFrame (same 4 s inOut(ease) curve, repeat-reverse) | react-native-reanimated |
| Compute Toys | `async-mutex` replaced by `engine/mutex.ts` (same `acquire`/`isLocked`); the pan gesture is touch events | npm packages, gesture-handler |
| Import External Texture | `createTestVideoFrame`'s BGRA pattern in a GPUTexture instead of a decoded video | `createVideoPlayer` throws `ERR_WEBGPU_UNSUPPORTED` (no platform decoder, no IOSurface/AHardwareBuffer import in wgpu) |
| Sampler Parameters | no controls, as in the reference | the reference's `GUI` (`components/GUI.ts`) is a stub that draws nothing; it is kept as is |
| Diagnostics › Async Runner Starvation | a zero-delay timeout stands in for `requestIdleCallback` | NSH has no `requestIdleCallback` |
| Diagnostics › Device Lost Hang | reports "forceLossForTesting is unavailable" | Dawn's test helper has no wgpu equivalent; the reference says the same without it |
| Diagnostics › Device Destroy Before Detach | the raw-WebGPU mode only | the three.js mode needs three.js |
| Cubemap, Fractal Cube | the canvas follows the window's width instead of reading it once at module load | NSH's `windowSize()` is reactive |
| All | every entry shows on every platform | the reference hides Shadow Mapping and Sampler Parameters on iOS and VisionCamera on macOS |

Not ported (an explanation page each, `src/examples.tsx`, `src/demos/Diagnostics/DiagnosticsList.tsx`): E2E Tests,
Reanimated, Frame Pacing, Three.js, tensorflow.js, Shared Texture Memory, VisionCamera, View Snapshot, Native Buffer
View, 200 Icons, and the diagnostics Worklet requestAdapter, Runtime Reload Lifecycle, Android View Props.

## Run-time downloads

As in the reference, MNIST Inference downloads its weights from TypeGPU's site and Compute Toys downloads toy 2349
from compute.toys when the demo opens. The tests stub both (`gallery.test.tsx`: a 784-10-...-10 network that answers
7 for any stroke, a UV-gradient toy); a real run needs the network.
