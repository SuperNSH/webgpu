# Audit: nsh-webgpu at `a9e7e4d`

Independent audit before publishing as `github.com/SuperNSH/webgpu`, 2026-11-24. Gate logs, repro crate and greps are in `docs/evidence/audit/`.

## Verdict: ready after fixes

All gates pass (fmt, clippy on host + wasm32/android/windows-gnu/ios-sim, 114 cargo tests, `nsh check`/`build` with 0 warnings and `nsh test` 82/1/45 in headless/canvas-apple/gallery; warm caches, Linux not cross-checked). No unsound `unsafe`, no data race, no panic from JS input past `guard`.

## Findings

| ID | Sev | Where | Summary |
|---|---|---|---|
| B1 | blocker | `LICENSE`, `tsx/*`, `examples/gallery/src/**` | Ported react-native-webgpu code (MIT, William Candillon) and webgpu-samples shaders (BSD-3) ship without their notices; the Stanford dragon data has no terms |
| I1 | important | `tsx/api/GPUDevice.ts:80-84,112-115`, `src/api/device.rs:196-214` | No GPUDevice is ever released; `destroy()` doesn't release; listener thread-locals hold `this` strongly |
| I2 | important | `tsx/Canvas.tsx:210-265`, `canvas_context.rs:513` | Ref handed out before the view exists; `getContext()` then gives a 1x1 canvas. Reference sets the ref after mount and throws "Cannot get context before mount" |
| I3 | important | `tsx/hooks.tsx:70-83` | `useSurface().surface` computed once before mount: always `0n` with a real `<Canvas>` (test uses a fake canvas) |
| I4 | important | `tsx/Canvas.tsx:245-250`, `hooks.tsx:54-63`, `canvas_context.rs:446-450` | Function refs not cleared on unmount; a later `getContext` re-creates a native context that is never retired |
| I5 | important | `tsx/types/dom.d.ts`, `tsx/index.tsx` | The reference's `declare global` augmentations (nativePointer, dawnToggles, importSharedTextureMemory/Fence, drawElementImageToTexture, native copyExternalImageToTexture/importExternalTexture, rotation/mirrored, GPUExternalTexture.destroy) are missing |
| N1 | nit | `src/api/buffer.rs:108,218` | Unchecked u64 `offset+size` in the Rust API |
| N2 | nit | `src/error.rs:222-245` | Off-thread errors land in the JS thread's open error scope |
| N3 | nit | `canvas_context.rs:316-336,713-735` | UI-thread attach and first `current_texture` may both configure the surface |
| N4 | nit | `src/surface/android.rs:573-577` | Old ANativeWindow released before its surface is replaced |
| N5 | nit | `error.rs:317-322`, `gpu.rs:19-22` | `pollster::block_on` in wasm in-place `blocking` would hang |
| N6 | nit | `src/api/gpu.rs:108` | `Wgpu.release` not wrapped in `guard` |
| N7 | nit | `gpu.rs:93-103`, `pending.rs` | Ticket kept after `wait` gives up |
| N8 | nit | `src/surface/offscreen.rs:342` | One OS thread per Linux frame readback |
| N9 | nit | `src/surface/views.rs:38-47` | Replaced surface dropped under the VIEWS lock |
| N10 | nit | `tsx/Canvas.tsx:190-208` | Warnings not dev-gated; Android zOrder/HardwareBufferView warnings missing |
| N11 | nit | `Cargo.toml` | `cargo package` fails on git deps; readme/categories/documentation missing |

## Parity

Every runtime export of the reference is present. Gaps: I5; `NativeProps` type missing; ref lifecycle (I2, I4); `CanvasProps` without `children`; an extra third parameter on `createTestVideoFrame`. 14 of 15 sampled `docs/parity.md` rows hold; the hooks' "done" is overstated (I3, I4).

## Docs inaccuracies

1. README says gates weren't recorded on the merged tree and merged wasm/Android clippy is unknown: stale.
2. README and installation guide say merged code wasn't run on Android: contradicts `parity.md`/`architecture.md` and integration evidence.
3. The Android screenshot's Triangle canvas is black (COPY_SRC capture); undocumented.
4. README Layout calls the gallery "in progress".
5. README sample's layout delay is the I2 workaround.
6. Installation guide could use `--template mixed`.

## Publish readiness

About 30 MB of evidence logs/PNGs tracked; `/Users/hmziq` paths only in evidence files; `AGENTS.md`, `CONTRIBUTING.md`, `PLAN.md`, `inventory.md` describe private local checkouts; no secrets; `.gitignore` fine; crates.io blocked by git deps (N11).
