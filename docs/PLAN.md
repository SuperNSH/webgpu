# Plan

Port `react-native-webgpu` ([`wcandillon/react-native-webgpu`](https://github.com/wcandillon/react-native-webgpu/tree/06599f9), commit 06599f9, 2026-11-22) to NSH as the nshm
`nsh-webgpu`.

## Architecture (target)

| react-native-webgpu | nsh-webgpu |
|---|---|
| Dawn (C++), JSI host objects in `cpp/rnwgpu/api/*` | `wgpu` 29 in the root crate `nsh-webgpu`, exposed to TSX by `#[native_module("nsh-webgpu")]` |
| `navigator.gpu`, `GPUAdapter`, `GPUDevice`, ... (spec objects) | Spec-shaped TS classes in `packages/nsh-webgpu/src/api/*` over a handle-based Rust module; `installWebGPU()` puts `navigator.gpu` and the `GPU*` globals in place |
| `<Canvas>` (Fabric `WebGPUView`: CAMetalLayer / SurfaceView / TextureView) | `<Canvas>` on an NSH native component `webgpu.canvas`, registered per platform with `#[native_components(...)]`, surface made by `wgpu::Instance::create_surface_unsafe` from the platform's raw handle |
| `canvasRef.current.getContext("webgpu")` -> `RNCanvasContext` (`present()`) | Same shape: `GPUCanvasContext` with `configure`, `unconfigure`, `getConfiguration`, `getCurrentTexture`, `present` |
| `useDevice`, `useCanvasRef`, `useSurface`, `GPUDeviceProvider`, `useMainDevice`, `warnIfNotHardwareAccelerated` | Same names, written for NSH's signals/JSX (`nsh` has no React; map hooks onto NSH primitives and keep the call shapes) |
| `GPUOffscreenCanvas` | Same, texture-backed, with readback |
| `requestAnimationFrame` / `cancelAnimationFrame` | Same names, driven by NSH's frame clock |
| `createImageBitmap(ArrayBuffer)`, `copyExternalImageToTexture` | Same, decoding with the `image` crate |
| `NativeBuffer`, `NativeBufferView`, `createVideoPlayer`, `importSharedTextureMemory`, `importSharedFence`, `importDevice`, `adoptTexture`, `drawElementImageToTexture`, Dawn toggles | Names kept; implemented where wgpu/NSH allow, else `ERR_WEBGPU_UNSUPPORTED` and a row in `docs/parity.md` |
| Example app `apps/example/src/*` (Triangle, Cube, ComputeBoids, Particles, ShadowMapping, ...) | `examples/gallery` NSH app with the portable demos |
| Jest spec tests `src/__tests__/*.spec.ts` (headless, compare images) | `nsh test` + `cargo test` that render offscreen and compare pixels |

## Layout (target)

```text
Cargo.toml                      workspace
Cargo.toml, src/ (repo root)     the Rust nshm (moved from crates/nsh-webgpu in phase 4a) (wgpu, handle registry, native module, native components)
nsh.json (repo root)            TSX exports of the crate: "nsh-webgpu" -> tsx/index.tsx
packages/nsh-webgpu/            TSX: spec classes, Canvas, hooks, tests (nsh test)
examples/gallery/               NSH app with the demos
docs/                           PLAN.md, architecture.md, parity.md, evidence/
```

## Phases (dynamic: each phase's briefs are written from the last phase's reports)

1. **Architect** (one agent): workspace, crate, package, handle registry, the minimal path adapter -> device ->
   buffer -> shader -> render pipeline -> encoder -> render pass -> queue.submit -> readback, headless, tested in Rust
   and TSX. Writes `docs/architecture.md` and fixes the file layout the next phase splits up.
2. **Fan-out** (parallel, disjoint areas): resources, commands, surfaces (Apple / others), React-API parity.
3. **Content**: examples gallery, spec-test port, docs + README + CI.
4. **Audit**: an independent review and gate run; a fix round on what it finds.
5. **Publish**: the orchestrator commits and creates `SuperNSH/webgpu`.

## Ownership

Phase 2 areas each work in their own git worktree `<checkout>-wt/<area>` (branch `area/<area>`), merged by the orchestrator. File ownership is the table at the end of the architect's report, mirrored in each brief: (a) resources, (b) commands, (c) surfaces-apple, (d) surfaces-other, (e) react-parity; foundation files are orchestrator-owned.

## Log

- Phase 0 (orchestrator): repo, nightly NSH in `.nsh-nightly`, this plan.
- Phase 1 (architect): workspace, crate, 26 spec classes, headless Triangle in Rust and `nsh test`; all gates green on Metal. Inventory written (`docs/inventory.md`).
- Phase 2: five areas launched in parallel worktrees.
