# Architecture

nsh-webgpu is react-native-webgpu for Native Shell (NSH): the same JavaScript API (`navigator.gpu`, the spec's
`GPU*` classes, `<Canvas>`, the hooks), on [wgpu](https://wgpu.rs) 29 where the reference uses Dawn. This page is the
design phase 1 settled, the canvas contract phase 2 built, and what phase 3 (integration) joined; see "What is
verified" for where it runs.

## Layout

```text
Cargo.toml                         the nshm's crate `nsh-webgpu` (keywords = ["nshm"], MIT) and its workspace
                                   (members = ["."]; examples/ and docs/upstream/ excluded); nsh crates from Git
                                   at 2964346, wgpu 29
nsh.json                           { "exports": { "nsh-webgpu": "tsx/index.tsx" } }
src/lib.rs                         #[native_module("nsh-webgpu/native")]: the module list
src/registry.rs                    the handle registry
src/error.rs                       exceptions (NativeError) and GPU errors (ErrorSink, scopes)
src/convert.rs                     descriptor parsing, shared dictionaries (Extent3D, Origin3D, Color, texel copies)
src/bytes.rs                       Bytes: ArrayBuffer / ArrayBufferView in
src/pending.rs                     tickets for awaited work (mapAsync, onSubmittedWorkDone)
src/instance.rs                    the process's wgpu::Instance
src/api/<interface>.rs             one #[module] per spec interface (WgpuBuffer, WgpuTexture, ...)
src/surface/                       the canvas: views.rs (contract), <platform>.rs, entries.rs
tests/native/                      cargo tests: the module called as TSX calls it, on the GPU
tsx/index.tsx                      the package entry: export list, installWebGPU() on import
tsx/api/<Interface>.ts             one spec class per file (GPUBuffer.ts, ...), errors.ts, constants.ts
tsx/internal/                      native.ts (module + error translation), object.ts (ids), wire.ts (descriptors)
tsx/types/                         webgpu.d.ts (@webgpu/types 0.1.74, vendored), dom.d.ts, spec.ts
tsx/*.ts(x)                        react-native-webgpu's own exports (Canvas, hooks, Offscreen, video, ...)
examples/headless/                 TSX app + nsh tests that use the package as an app does (nsh add --path)
examples/canvas-apple/             <Canvas> on macOS / iOS (Triangle, Cube)
examples/canvas-native/            the webgpu.canvas view on Android / Windows / Linux, drawn from the app's crate
examples/canvas-web/               the web canvas in a browser (TSX only)
examples/gallery/                  the reference's example app: its demos and diagnostics
docs/evidence/<area>/              gate outputs and pictures per area (architect, the five areas, integration)
docs/upstream/                     standalone repros of upstream bugs (excluded from the workspace)
```

The crate is the repository's root because `nsh add` reads an nshm at the root of what it gets: `--path <dir>`
reads `<dir>`, and a Git source (`github:SuperNSH/webgpu`) is a bare clone whose commit is `git archive`d and unpacked
whole into `$NSH_HOME/nshms/github.com/SuperNSH/webgpu/<version>/`, then read at that directory's root
(`nsh-cli/src/nshm/fetch.rs` `Net::install`, `manifest.rs` `Manifest::read`: `Cargo.toml`'s `[package]` name and
version, a `#[native_module]` in `src/lib.rs`, and `nsh.json`'s `"exports"`, files inside the directory). A root
that is only a `[workspace]` fails with `no [package] name` (`docs/evidence/docs-ci/nsh-add-probe.txt`, before the
move); this layout passes (`docs/evidence/fixes/nsh-add-root.txt`).

The TSX lives inside the crate because the nightly refuses an nshm export outside the nshm's directory
(`nsh-cli/src/nshm/manifest.rs`: `file.split(['/', '\\']).any(|p| p == "..")`), and an export may not share the
Rust module's specifier (same file: "is its Rust module's specifier too"). Hence `nsh-webgpu` is the TSX entry and
`nsh-webgpu/native` the Rust module. PLAN.md's `packages/nsh-webgpu/src` is `tsx/`.

## Binding design

1. **Handles.** Every WebGPU object is a `u32` id into one process-wide registry (`src/registry.rs`:
   `Mutex<BTreeMap<u32, Box<dyn Any + Send>>>`). Ids are unique across kinds, so a descriptor names any object by
   number and the reader downcasts (`registry::get::<BufferEntry>(id)`; the wrong kind is a `TypeError`). Each kind's
   entry type (`BufferEntry`, `TextureEntry`, ...) lives in its own `api/*.rs`; the registry knows none of them.
2. **One native module per interface.** `WgpuBuffer.create(device, desc)`, `WgpuBuffer.mapAsync(...)`,
   `WgpuRenderPassEncoder.draw(...)`: plain functions taking ids, a descriptor and bytes. A device's `create*` lives
   in the file of the kind it creates, so `device.rs` never grows. Rust callers use the same functions.
3. **Spec classes in TSX.** `tsx/api/GPUBuffer.ts` is `class GPUBuffer extends ObjectBase implements
   SpecGPUBuffer`: it holds the id (`[HANDLE]`), the label, what the spec reports from the descriptor (`size`,
   `usage`, `width`, `format`, ...), and the content-timeline state (`mapState`). Every class `implements` the
   @webgpu/types interface, so `nsh check` fails on any drift from the spec's shapes. `new GPUBuffer()` throws
   `TypeError: Illegal constructor` (an `INTERNAL` token guards the constructors). `installWebGPU()` (run by the
   entry on import, as the reference's native install) puts `navigator.gpu`, the classes and the flag constants on
   `globalThis`; `instanceof` works against both the import and the global.
4. **Descriptors** cross as plain objects (`JsonValue`): `wire()` replaces each GPU object with its id, iterables
   with arrays, and drops `undefined`. Rust deserializes them with serde into spec-named structs; wgpu's own enums
   read the spec's strings (wgpu feature `serde`: `"triangle-list"`, `"rgba8unorm"`, `"2d-array"`). A shape that
   does not convert is a `TypeError`, as WebIDL conversion is. Numbers arrive as integers when whole (nsh-js
   `protocol.rs` `to_json`).
5. **Bytes** in are `Bytes` (`ArrayBuffer | ArrayBufferView`, read with `JsIn::as_buffer`, the view's range only);
   bytes out are `Vec<u8>` (a fresh `Uint8Array`). `writeBuffer`'s `dataOffset`/`size` are counted in elements in
   TSX (`bytesOf`) before crossing.
6. **Mapping is copy-in/copy-out.** `getMappedRange` returns a fresh `ArrayBuffer` holding a copy of the mapped bytes
   (wgpu's `get_mapped_range`); `unmap()` writes each range back with `WgpuBuffer.writeMapped` when the mapping is
   for writing (`mappedAtCreation` or `MAP_WRITE`), then detaches the ArrayBuffers (`ArrayBuffer.prototype.transfer`)
   as the spec does. No JS object ever points into memory wgpu may unmap. Overlap, range and alignment checks throw
   `OperationError`. Cost: one copy each way of the mapped range.
7. **Async.** `requestAdapter` / `requestDevice` are `async fn`s (NSH runs them as core tasks; wgpu's futures run on
   `blocking` threads). Work that must start in order with the JS calls (`mapAsync`, `onSubmittedWorkDone`) is split:
   a sync call starts it on the JS thread and returns a ticket (`src/pending.rs`); `await Wgpu.wait(ticket)` polls
   the device on a blocking thread until wgpu's callback fills the ticket. `mapAsync` and `onSubmittedWorkDone`
   first `queue.submit([])`, because wgpu holds `writeBuffer` data until the next submit while WebGPU's queue
   timeline orders it before the map.
8. **GPU errors** never panic. Each device installs wgpu's `on_uncaptured_error`, which hands every error to the
   device's `ErrorSink` (`src/error.rs`). The sink keeps the spec's error-scope stack itself (per device; wgpu's own
   scopes are per thread and their guards `!Send`), keeps the first error per scope, and sends an error no scope
   catches to the device's `uncapturederror` listener: a TSX callback kept in a thread-local, posted when the error
   happens on the JS thread (every sync call), queued otherwise and drained by TSX after each wait
   (`device.drainErrors()`). The binding reports its own validation errors the same way (a command buffer submitted
   twice, a finished encoder, writeBuffer out of range: things wgpu panics on). TSX dispatches
   `GPUUncapturedErrorEvent` (and `onuncapturederror`), and warns on the console when nobody listens, as browsers
   do. `createRenderPipelineAsync` / `createComputePipelineAsync` read the descriptor on the JS thread and compile
   on a blocking thread (`nsh_core::blocking`) inside wgpu's own validation and internal scopes, which are per
   thread, so the JS thread keeps running and the device's scopes never see the pipeline's errors; an invalid
   pipeline rejects with `GPUPipelineError` (`src/error.rs` `in_pipeline_scopes`).
9. **Exceptions** cross as `Err(String)` = `"<Name>: <message>"`; `tsx/internal/native.ts` rethrows them as
   `TypeError`, `RangeError`, `DOMException(name)` (`OperationError`, `AbortError`, ...), or an `Error` with
   `code: "ERR_WEBGPU_UNSUPPORTED"`. Every native call is wrapped in `guard()`, which turns a wgpu panic into an
   `OperationError` instead of unwinding into QuickJS.
10. **Lifetime.** A `FinalizationRegistry` releases an object's id (`Wgpu.release`) when JS collects it; the wgpu
    object lives on while other objects or submitted work hold it. A device's native listeners hold its `GPUDevice`
    weakly, so an undestroyed device the app drops is collected and released too (`WgpuDevice.release`).
    `device.lost` settles on `destroy()` at once, and from wgpu's lost callback otherwise; `destroy()` then
    releases the device's id and listeners, and keeps the destroyed handle aside (`RETIRED` in `api/device.rs`) for
    its objects until the `GPUDevice` is collected.

The macro needs the literal `Result<T, String>` in signatures (it matches the type's name), so the alias
`NativeResult` is for internal helpers only.

## Values at the boundary

| WebGPU | crosses as | Rust |
|---|---|---|
| a GPU object | its id | `u32`, `registry::get::<Entry>` |
| a descriptor / dictionary | plain object, objects as ids (`wire`) | `JsonValue` → `convert::parse::<T>` (serde) |
| `GPUExtent3D`, `GPUOrigin3D`, `GPUColor` | array or dict | `convert::{Extent3d, Origin3d, Color}` |
| `AllowSharedBufferSource` | `ArrayBuffer` / view | `Bytes` (copied) |
| bytes back (`getMappedRange`) | `Uint8Array` → its `ArrayBuffer` | `Vec<u8>` |
| `Promise` | `async fn`, or ticket + `Wgpu.wait` | `pending::{start, fill}` |
| events (`uncapturederror`, `lost`) | `Callback<(String, String)>` | thread-local listeners, `post` |
| flags (`GPUBufferUsage`) | number | `u32`, checked against the spec's bits |

## Tests

- `cargo test --workspace`: `tests/native/` calls the module's functions as TSX does on the
  machine's GPU: the reference's Triangle (`apps/example/src/Triangle`) rendered offscreen in the preferred canvas
  format and checked pixel by pixel (`NSH_WEBGPU_EVIDENCE=<dir>` saves `triangle-rust.png`), mapping round trips,
  error scopes, uncaptured errors, `TypeError`/`RangeError`/`OperationError`/`AbortError`, `destroy()`.
- `nsh test` in `examples/headless`: the same Triangle through the TSX classes (`App.tsx` shows what it read back,
  `App.test.tsx` checks it), and `webgpu.test.tsx`, which drives the classes directly in the test runner
  (`instanceof`, illegal constructors, set-like features, limits, labels, error scopes, `uncapturederror`,
  `GPUPipelineError`, mapping and detaching, `writeBuffer` element offsets, `lost`).
- **`nsh test` reaches the GPU on this Mac.** The headless runner nsh builds with the crate is a plain macOS process;
  wgpu finds the Metal adapter there, with no window (`docs/evidence/architect/nsh-test.txt`).
- Each area adds its own test binary (`tests/<area>/main.rs`) and test file
  (`examples/headless/<area>.test.tsx`); neither list is shared.

## Canvas

```text
<Canvas> (tsx/Canvas.tsx)
  ├─ web: <View><WebCanvasView contextId/></View>          tsx/surface/web.tsx: a <canvas> in the view's element,
  │                                                         the browser's navigator.gpu and context (no native calls)
  └─ else: <View {...props}><webgpu.canvas contextId android* apple* style={{flex: 1}}/></View>
       contextId = WgpuCanvasContext.allocateContextId()   unique for the process (views outlive a JS reload)
       platform view, UI thread (surface/<platform>.rs, #[native_components(<platform>)])
         a surface:   views::attach / resize / detach (surface/views.rs)
                      + canvas_context::set_native (pointer, size in points, scale)
                      + canvas_context::surface_attached (after attach) / view_gone (after detach)
         no surface:  offscreen::show / resize / hide (surface/offscreen.rs, Linux) + set_native / view_gone
ref(canvas) after the inner view's first layout; ref(null) on unmount (then getContext throws, and
  WgpuCanvasContext.destroyContext retires the id: make() refuses it from then on)
ref.getContext("webgpu") → GPUCanvasContext (tsx/api/GPUCanvasContext.ts)
  configure / getCurrentTexture / present / unconfigure → WgpuCanvasContext (src/api/canvas_context.rs)
```

**The contract** (`src/api/canvas_context.rs`, `src/surface/views.rs`, `src/surface/offscreen.rs`). The canvas
context owns each context's state, by context id, on every platform: its configuration, the frame in flight, an
offscreen texture. A platform view tells it what it has:

| Call (UI thread) | Meaning |
|---|---|
| `views::attach(id, surface, w, h)`, then `canvas_context::surface_attached(id)` | a `wgpu::Surface<'static>` (from `crate::instance()`) shows the context; a configured context configures it at once and blits the frame presented offscreen meanwhile |
| `views::resize(id, w, h)` | the view's size in pixels changed |
| `canvas_context::view_gone(id)` | the view went: the frame in flight is set aside (JS may be drawing it; it ends at JS's next `present`, `getCurrentTexture` or `configure`, and keeps its surface until then), then the surface is detached and dropped; never panics (a panic in a `dealloc` aborts). The context renders offscreen from then on and goes once its `<Canvas>` unmounted and no context object holds it |
| `canvas_context::set_native(id, pointer, width, height, scale)` | what `getNativeSurface()` reports: the layer or window pointer, the size in points, the scale |
| `offscreen::show(id, presenter, w, h)` / `resize` / `hide` | a view with no surface of its own shows the context's frames (Linux) |

Every surface platform (Apple, Android, Windows) makes these calls; Linux makes `offscreen::show` and
`set_native` (no pointer). `configure` fits the canvas to the surface: an `alphaMode` the surface does not offer is
replaced by one it does, and a canvas whose format, usage or view formats it lacks (Android's GL surfaces: render
attachment only, no view formats) renders offscreen and is drawn onto it at `present()` (`Shown`, wgpu's
`TextureBlitter`); the surface's capabilities come from the device's adapter, which the device entry keeps. A surface
configured with another device first lets go of the swap chain of the one before (`release_swap_chain`).

Where a frame goes: a context whose id has a surface in `views` presents it (`SurfaceTexture::present`); one with
none renders into its own offscreen texture, and `present()` hands that texture to `offscreen::deliver`, which reads
it back off the JS thread for a presenting view when one is shown, and keeps it for a surface that attaches later.
`views.rs` holds surfaces only (decided in phase 3): a presenting view has no `wgpu::Surface`, so it stays beside
the readback it needs in `offscreen.rs`, and the canvas context asks both registries. The drawing buffer is the
canvas's `width` × `height` everywhere (the spec's rule), not the view's pixels: GTK's picture scales it, as Core
Animation scales a drawable. `offscreen.rs` also keeps its own context functions (`WgpuOffscreenCanvas.configure`,
`currentTexture`, `present`) for Rust callers that want a context with no `<Canvas>`, and the readback that
`WgpuOffscreenCanvas.readPixels` / `takePixels` give TSX.

| Platform | View | Surface |
|---|---|---|
| macOS | `NSView` hosting a `CAMetalLayer` (`surface/macos.rs`, `apple.rs`) | `SurfaceTargetUnsafe::CoreAnimationLayer`; drawable = bounds × backing scale |
| iOS | `UIView` whose `+layerClass` is `CAMetalLayer` (`surface/ios.rs`, `apple.rs`) | same |
| Android | `FrameLayout` with a `TextureView` (default) or `SurfaceView` (`surface/android.rs`); lifecycle followed on `AChoreographer` (no Java in an nshm) | `ANativeWindow_fromSurface` → `RawHandle` (AndroidNdk); the instance has one backend (`instance.rs`) |
| Windows | a DirectComposition visual (`surface/windows.rs`); a child `HWND` without a DX12 adapter or with `WGPU_BACKEND` set | `SurfaceHandle` (DX12), else `RawHandle` (Win32) |
| Linux | `gtk::Picture` (`surface/linux.rs`) | none: offscreen frames read back, shown as `gdk::MemoryTexture` |
| Web | `<canvas>` in the view's element (`tsx/surface/web.tsx`) | the browser's WebGPU context, no wgpu |

`instance.rs`, `request_adapter`: on Linux and Android adapters are enumerated once and kept. wgpu-hal's GL adapters
share one EGL context but each locks it apart, so enumerating again while another thread enumerates, drops an adapter
or uses a GL device panicked (`EGL_BAD_ACCESS`).

`instance.rs`: on Android an `ANativeWindow` takes one producer API, and a surface made for Vulkan and GL at once
connects the window for Vulkan, so GL's EGL surface fails. The instance there has Vulkan when it yields an adapter,
else GL (emulators), unless `WGPU_BACKEND` is set (`android_backends`, tested on the host).

## Blockers and limits in the nightly

- **The app's NSH and this crate's have to be one commit.** An installed nightly builds an app whose only Rust is an
  nshm against its own commit (longbridge/nsh#2980, since 2026-11-24), so with the nightly this crate is pinned to the
  app needs no crate. A released CLI takes crates.io's `nsh-core`, and one built in a checkout takes that checkout by
  path: two `nsh-core`s, E0308 (`expected Registry, found nsh_core::element::component::Registry`). Then the app has a
  crate of its own pinned at the same revision (`examples/headless/native`, `nsh.json` `"rust": "native"`), and nsh
  takes the Git checkout for everything. Once nsh-webgpu depends on crates.io `nsh-core`, this goes away.
- `nsh add --path` reads the version with a line parser (`nshm/manifest.rs` `package_version`): `version.workspace =
  true` is "Cargo.toml has no [package] version", so the crate states its version.
- `#[module]` recognizes `Result` by name: a type alias returning `Result` fails to compile with "`Result<..>: Ret`
  is not satisfied".
- Cargo resolves newer patch versions than NSH tested (rolldown_std_utils 1.3.3 broke nsh-bundler 1.2.13): the
  workspace's `Cargo.lock` was seeded from NSH's own and moves with the pin; keep it, and seed `examples/headless/native` the same.
- **The web player runs a TSX app's modules in a same-origin iframe realm** (`nsh-platform-web/web/browser/realm.js`)
  while the views are elements of the page: `globalThis.document` is the iframe's, empty; `tsx/surface/web.tsx` uses
  `parent.document` / `parent.navigator.gpu`. The first `onLayout` comes before the DOM runtime inserted the view's
  element, so the `<canvas>` is looked for again every 16 ms (up to a second). NSH shows no registered native
  component on the web, hence the `<canvas>` inside a plain view. (surfaces-other)
- **An nshm ships no Java**: Android's canvas cannot implement `SurfaceHolder.Callback` or the reference's
  `HardwareRenderer` path, so the surface lifecycle is polled on `AChoreographer` each vsync and `HardwareBufferView`
  falls back to a `TextureView`. (surfaces-other)
- **GTK 4 widgets have no native window or `wl_surface`**: Linux shows offscreen frames read back each `present()`
  (one GPU→CPU→GPU copy a frame; wgpu 29 cannot export a texture as dmabuf). (surfaces-other)
- **Windows without DX12**: the canvas is a child `HWND`, opaque and above whatever NSH draws over it (airspace).
  With DX12 it is a composition visual and has neither limit. (surfaces-other)
- **macOS window pictures**: `app.screenshot()` returns a black picture and `screencapture` fails without Screen
  Recording permission, so the macOS canvas is verified by reading its drawables back. (surfaces-apple)
- **NDK 29.0.14206865's `llvm-ar` on this Mac execs itself**: `nsh run --target android` hangs in `zstd-sys`'s build;
  `ANDROID_NDK_HOME=.../ndk/28.2.13676358` works. (surfaces-other)
- **clippy's `missing_const_for_thread_local` fires on Android** for `thread_local!`s that are already `const`
  (`api/device.rs`, `error.rs`); allowed there with the reason. (integration)
- The headless test runner has no PNG decoder: pixel checks against the reference's snapshots run in cargo.
  (resources, commands)

## Phase 2: ownership (history)

Phase 3 (integration) edits any file; this was the phase 2 split. Disjoint file sets. "Orchestrator" files are the frozen foundation: an area that needs a change there says so in its
report. New files go in the owner's directories below; nothing else is shared.

| Area | Rust (`src/`) | TSX (`tsx/`) | Tests and docs |
|---|---|---|---|
| **orchestrator** | `lib.rs`, `registry.rs`, `error.rs`, `convert.rs`, `bytes.rs`, `pending.rs`, `instance.rs`, `api/mod.rs`, `api/gpu.rs`, `api/adapter.rs`, `api/device.rs`, `surface/mod.rs`, `surface/entries.rs`, `surface/views.rs`; `Cargo.toml` (root and crate), `Cargo.lock`, `nsh.json` | `index.tsx`, `install.ts`, `internal/*`, `types/*`, `api/GPU.ts`, `api/GPUAdapter.ts`, `api/GPUDevice.ts`, `api/errors.ts`, `api/constants.ts` | `tests/native/*`, `examples/headless/{App.tsx,App.test.tsx,triangle.ts,webgpu.test.tsx,nsh.json,nsh.lock,native/}`, `docs/architecture.md`, `docs/PLAN.md`, `docs/parity.md` (merged from the areas' files) |
| **(a) resources** | `api/buffer.rs`, `api/texture.rs`, `api/texture_view.rs`, `api/sampler.rs`, `api/bind_group_layout.rs`, `api/bind_group.rs`, `api/pipeline_layout.rs`, `api/shader_module.rs`, `api/render_pipeline.rs`, `api/compute_pipeline.rs`, `api/query_set.rs`, `api/features.rs`, `api/limits.rs` | `api/GPUBuffer.ts`, `api/GPUTexture.ts`, `api/GPUTextureView.ts`, `api/GPUSampler.ts`, `api/GPUBindGroupLayout.ts`, `api/GPUBindGroup.ts`, `api/GPUPipelineLayout.ts`, `api/GPUShaderModule.ts`, `api/GPURenderPipeline.ts`, `api/GPUComputePipeline.ts`, `api/GPUQuerySet.ts`, `api/GPUSupportedFeatures.ts`, `api/GPUSupportedLimits.ts`, `api/GPUAdapterInfo.ts` | `tests/resources/`, `examples/headless/resources.test.tsx`, `docs/parity/resources.md` |
| **(b) commands** | `api/command_encoder.rs`, `api/command_buffer.rs`, `api/render_pass_encoder.rs`, `api/compute_pass_encoder.rs`, `api/render_bundle_encoder.rs`, `api/render_bundle.rs`, `api/queue.rs`, `api/image_bitmap.rs`, `api/external_texture.rs` | `api/GPUCommandEncoder.ts`, `api/GPUCommandBuffer.ts`, `api/GPURenderPassEncoder.ts`, `api/GPUComputePassEncoder.ts`, `api/GPURenderBundleEncoder.ts`, `api/GPURenderBundle.ts`, `api/GPUQueue.ts`, `api/GPUExternalTexture.ts`, `imageBitmap.ts` | `tests/commands/`, `examples/headless/commands.test.tsx`, `docs/parity/commands.md` |
| **(c) surfaces-apple** | `api/canvas_context.rs`, `surface/apple.rs`, `surface/macos.rs`, `surface/ios.rs` | `api/GPUCanvasContext.ts`, `Canvas.tsx`, `animationFrame.ts` | `tests/surfaces_apple/`, `examples/headless/surfaces-apple.test.tsx`, `docs/parity/surfaces-apple.md`, simulators `nsh-webgpu-*` |
| **(d) surfaces-other** | `surface/windows.rs`, `surface/linux.rs`, `surface/android.rs`, `surface/web.rs`, `surface/offscreen.rs` | (asks (c) for `Canvas.tsx` changes; a platform variant goes in a new file it owns) | `tests/surfaces_other/`, `examples/headless/surfaces-other.test.tsx`, `docs/parity/surfaces-other.md`, AVDs `nsh-webgpu-*` |
| **(e) react-parity** | none (asks (b)/(c) for native pieces) | `hooks.tsx`, `GPUDeviceProvider.tsx`, `Offscreen.ts`, `formats.ts`, `video.ts`, `importDevice.ts`, `NativeBufferView.tsx`, `WebGPUModule.ts`, `types.ts` | `examples/headless/react-parity.test.tsx`, `docs/parity/react-parity.md` |

Rules that keep the sets disjoint:

- A native function goes in the file of the interface it belongs to (`setVertexBuffer` in `render_pass_encoder.rs`);
  `lib.rs` already registers every module, and `index.tsx` already exports every spec name (the react-native-webgpu
  files are `export *`, so (e) adds names without touching it).
- `GPUDevice.ts` calls `create*` factories exported by each kind's file (`createSampler` in `GPUSampler.ts`); an area
  changes the factory, never `GPUDevice.ts`.
- Shared descriptor pieces an area needs beyond `convert.rs` go in its own file (`pub(crate)`), e.g. a
  `GPUBindGroupEntry` parser in `bind_group.rs`; ask the orchestrator to move it into `convert.rs` later.
- Dependencies (`objc2-*`, `jni`, `windows`, nsh platform crates, `image`) are requested in the area's report; the
  orchestrator adds them to the root `Cargo.toml` and `Cargo.lock`.
- Each area keeps `docs/parity/<area>.md` (rows: export, status, reason); the orchestrator merges them into
  `docs/parity.md`.

## What is verified

On macOS 27.2, Apple Silicon, Metal, nightly `nsh 0.6.0 (2964346 nightly 2026-11-24)`, rustc 1.99.1, after phase 3
(`docs/evidence/integration/`): `cargo fmt`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test
--workspace`; clippy `-D warnings` for `wasm32-unknown-unknown`, `aarch64-linux-android`, `x86_64-pc-windows-gnu`,
`aarch64-apple-ios-sim` and (GTK faked) `x86_64-unknown-linux-gnu`; `nsh check` / `nsh build` / `nsh test` in
`examples/headless`, `canvas-apple`, `canvas-native`, `canvas-web`. Each phase 3 change has a test seen failing on
purpose (`broken-on-purpose-*.txt`). On screen: the iOS simulator and a browser (area evidence) and an Android
emulator (GL; `android-canvas-native.png`, `android-canvas-apple.png`: the view, and `<Canvas>` with the rAF Cube). The state today is in
the README's Status: every platform on screen, the desktops in CI (`.github/workflows/onscreen.yml`). Not verified: a
hardware GPU on Windows, Linux or Android, and a physical phone.
