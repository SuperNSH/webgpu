# nsh-webgpu

WebGPU for Native Shell (NSH) apps: a port of [react-native-webgpu](https://github.com/wcandillon/react-native-webgpu) on [wgpu](https://wgpu.rs) 29.

The same JavaScript API as the reference: `navigator.gpu` and the spec's `GPU*` classes, `<Canvas>` with `getContext("webgpu")` and `present()`, the hooks (`useDevice`, `useCanvasRef`, `useSurface`, `GPUDeviceProvider`, `useMainDevice`), `GPUOffscreenCanvas`, `requestAnimationFrame`, `createImageBitmap`. Where react-native-webgpu runs Dawn, nsh-webgpu runs wgpu, which NSH already uses. Exports wgpu cannot back (Dawn device import, platform buffers, video) keep their names and throw an error with `code: "ERR_WEBGPU_UNSUPPORTED"`; [docs/parity.md](docs/parity.md) lists each with the reason.

Part of the SuperNSH capability set. Guides in [docs/guide/](docs/guide/README.md), CI in `.github/workflows/ci.yml`.

## Status

**Pre-release (0.1.0, unpublished). Verified on macOS (Metal) and the iOS simulator, on screen on an Android emulator (GLES) and in a browser; Windows and Linux are compile-checked only.** It needs the NSH nightly `f577c51`, not the 0.6.0 release.

| Platform | Backend | What was checked | Evidence |
|---|---|---|---|
| macOS 27.2, Apple Silicon | Metal | `cargo test` (117 tests); `nsh test` in `examples/headless` (89: spec classes, the reference's specs ported, pixel checks against its snapshots, the `<Canvas>` ref lifecycle), `examples/canvas-apple` (1) and `examples/gallery` (45) headless; the gallery's 45 tests in the macOS host (`nsh test --target macos`); the canvas's drawable read back from a `CAMetalLayer` | `docs/evidence/fixes-audit/` (phase 4c gates), `docs/evidence/surfaces-apple/` |
| iOS 27 simulator | Metal | `examples/canvas-apple` on screen (the Triangle and the rotating Cube); the gallery's tour | `docs/evidence/surfaces-apple/ios-simulator-screen*.png`, `docs/evidence/gallery/ios/` |
| Android emulator | GLES | the merged code on screen: `examples/canvas-native` (TextureView and SurfaceView) and `examples/canvas-apple` (`<Canvas>`, the Triangle and the rAF Cube) | `docs/evidence/integration/android-canvas-native.png`, `docs/evidence/fixes/android-canvas-apple.png` |
| Web | the browser's WebGPU | a `<Canvas>` drawing in Chrome (`nsh run --target web`) | `docs/evidence/surfaces-other/web-canvas-browser.png` |
| Windows | DX12 / Vulkan | Clippy for `x86_64-pc-windows-gnu`; never run | `docs/evidence/fixes-audit/clippy-x86_64-pc-windows-gnu.txt` |
| Linux | Vulkan / GL | Clippy cross-checked from macOS against a faked `pkg-config` (phase 3); never run | `docs/evidence/integration/clippy-x86_64-unknown-linux-gnu.txt` |

The gates were run on the current tree (phase 4c, `docs/evidence/fixes-audit/gates-rust.txt`, `gates-tsx.txt`): `cargo fmt`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace`, Clippy with `-D warnings` for `wasm32-unknown-unknown`, `aarch64-linux-android`, `x86_64-pc-windows-gnu` and `aarch64-apple-ios-sim`, and `nsh check`, `nsh build` (no warning) and `nsh test` in `examples/headless`, `examples/canvas-apple` and `examples/gallery`. The Android and iOS runs above are from phases 3 and 4a, before the phase 4c changes (the `<Canvas>` ref timing, device release); they were not repeated. CI (`.github/workflows/ci.yml`) has not run yet: the repository is not published.

On Android the canvases clear to black rather than showing the views under them: the emulator's GL surfaces offer only opaque compositing, so `alphaMode: "premultiplied"` falls back to opaque there (`docs/architecture.md`, "The contract"). The earlier screenshot `docs/evidence/integration/android-canvas-apple.png` also shows an empty Triangle canvas: the example's capture copied out of the surface texture, which Android's GL surfaces cannot do (no `COPY_SRC`), so that frame's submit was invalid; the example now skips the capture on Android, and `docs/evidence/fixes/android-canvas-apple.png` shows the triangle (the explanation is in `docs/evidence/fixes/android-canvas-apple.txt`).

Not verified at all: any GPU but an Apple one (and the emulator's GL translator), a picture of a macOS window (the drawable is verified, the compositing is not), a physical iPhone or Android device, Mac Catalyst, Windows and Linux at run time.

The gallery (`examples/gallery`) ports the reference's example app: its demos and diagnostics, with the differences listed in `examples/gallery/KNOWN_ISSUES.md`; its tests pass headless and in the macOS host.

One thing stands between an app and this package today: **the app needs a Rust crate of its own, pinned to the same NSH commit.** A TSX app whose only Rust is a Git-pinned nshm gets hosts built against crates.io's `nsh-core` 0.6.0, and the build fails with two `nsh-core`s (E0308). A crate of the app's own at `f577c51` makes nsh build everything from that checkout ([docs/architecture.md](docs/architecture.md), "Blockers"). This goes away once nsh-webgpu depends on a crates.io `nsh-core`.

The crate is the repository's root (its `Cargo.toml` and `nsh.json`), which is where `nsh add` reads an nshm, from a path or from Git: `nsh add --path` on a checkout succeeds (`docs/evidence/fixes/nsh-add-root.txt`). `nsh add github:SuperNSH/webgpu` reads the same root (`git archive` of the commit, unpacked whole), but it has not been run: the repository is not published yet.

## Install

You need Rust and the NSH nightly built from `f577c51` (`nsh 0.6.0 (f577c51 nightly 2026-11-22)`). The nightly release on GitHub is rebuilt from `main` every night, so it only matches while `main` has not moved: check `nsh --version`, or build the CLI from that commit (`cargo build --release --locked -p nsh-cli` in a checkout of `longbridge/nsh` at `f577c51`).

```sh
curl -fsSL https://nsh.dev/install.sh | sh -s -- --nightly
nsh --version   # nsh 0.6.0 (f577c51 nightly ...)
```

Add the package, in the app's directory, from GitHub (once published) or from a clone:

```sh
nsh add github:SuperNSH/webgpu
# or
git clone https://github.com/SuperNSH/webgpu ../webgpu
nsh add --path ../webgpu
```

`nsh.json` then has the dependency. Add `"rust"` for the app's own crate:

```json
{
  "name": "My App",
  "id": "com.example.myapp",
  "version": "1.0.0",
  "build": 1,
  "dependencies": {
    "nsh-webgpu": {
      "path": "../webgpu"
    }
  },
  "rust": "native"
}
```

`native/Cargo.toml`, the app's crate (copied from `examples/headless/native`):

```toml
[package]
name = "my-app-native"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
nsh-core = { git = "https://github.com/longbridge/nsh", rev = "f577c51901a2857956f8ba0580da4db9ce3a3311" }

[workspace]
```

`native/src/lib.rs`:

```rust
use nsh_core::prelude::*;

#[native_module("my-app/native")]
fn native(_registry: &mut Registry) {}
```

Copy `examples/headless/native/Cargo.lock` next to it too: it was seeded from NSH's own lock at `f577c51`, and newer patch releases of NSH's dependencies than that lock holds have broken the build before.

Once the repository root is the crate, the intended form is `nsh add github:SuperNSH/webgpu --rev <commit>` (or `"nsh-webgpu": { "github": "SuperNSH/webgpu", "rev": "<commit>" }` in `nsh.json`). That form is not verified.

## Example

The reference's HelloTriangle in a `<Canvas>`. Importing `nsh-webgpu` installs `navigator.gpu` and the `GPU*` globals.

```tsx
import { mount, pixelRatio, View } from "nsh";
import { Canvas, type CanvasRef } from "nsh-webgpu";

const triangleVertWGSL = `@vertex
fn main(
  @builtin(vertex_index) VertexIndex : u32
) -> @builtin(position) vec4f {
  var pos = array<vec2f, 3>(
    vec2(0.0, 0.5),
    vec2(-0.5, -0.5),
    vec2(0.5, -0.5)
  );

  return vec4f(pos[VertexIndex], 0.0, 1.0);
}`;

const redFragWGSL = `@fragment
fn main() -> @location(0) vec4f {
  return vec4(1.0, 0.0, 0.0, 1.0);
}`;

async function draw(canvas: CanvasRef) {
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("No adapter");
  const device = await adapter.requestDevice();
  const format = navigator.gpu.getPreferredCanvasFormat();

  // The canvas is laid out once its ref arrives: the context takes its size.
  const context = canvas.getContext("webgpu")!;
  context.canvas.width = context.canvas.clientWidth * pixelRatio();
  context.canvas.height = context.canvas.clientHeight * pixelRatio();
  context.configure({ device, format, alphaMode: "premultiplied" });

  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: { module: device.createShaderModule({ code: triangleVertWGSL }), entryPoint: "main" },
    fragment: {
      module: device.createShaderModule({ code: redFragWGSL }),
      entryPoint: "main",
      targets: [{ format }],
    },
    primitive: { topology: "triangle-list" },
  });

  const commandEncoder = device.createCommandEncoder();
  const passEncoder = commandEncoder.beginRenderPass({
    colorAttachments: [
      {
        view: context.getCurrentTexture().createView(),
        clearValue: [0, 0, 0, 0],
        loadOp: "clear",
        storeOp: "store",
      },
    ],
  });
  passEncoder.setPipeline(pipeline);
  passEncoder.draw(3);
  passEncoder.end();
  device.queue.submit([commandEncoder.finish()]);
  context.present();
}

function HelloTriangle() {
  return (
    <View style={{ flex: 1, backgroundColor: "#3498db" }}>
      {/* The ref gets the canvas once it is mounted, and null when it unmounts. */}
      <Canvas ref={(canvas) => canvas && void draw(canvas)} style={{ flex: 1 }} />
    </View>
  );
}

mount(HelloTriangle);
```

The ref is set once the canvas is mounted and laid out (and set to `null` when it unmounts), so `getContext()` in it sizes the canvas to the view, as in the reference. This block is `examples/headless/samples/readme.app.tsx` byte for byte (`cargo test --test readme` checks it): `nsh check` type-checks it and `samples.test.tsx` mounts it headless, where it draws offscreen; that test does not look at the pixels. The same drawing, as `drawTriangle` of `examples/canvas-apple/scenes.ts`, ran on screen in the iOS simulator and an Android emulator.

## Layout

- The root is the nshm: `Cargo.toml` (the `nsh-webgpu` package and its workspace; NSH crates from Git at `f577c51`, `wgpu` 29) and `nsh.json` (its TSX exports). `src/` is the Rust module `nsh-webgpu/native` (one `#[module]` per spec interface, the handle registry, the canvas views per platform in `src/surface/`); `tsx/` is the TSX package `nsh-webgpu` (the spec classes in `tsx/api/`, react-native-webgpu's own exports beside them, the vendored `@webgpu/types` in `tsx/types/`); `tests/` holds the cargo tests.
- `examples/headless/`: the TSX tests (`nsh test`), using the package as an app does.
- `examples/canvas-apple/`, `examples/canvas-native/`, `examples/canvas-web/`: the canvas on screen on Apple platforms, Android/Windows/Linux, and the web.
- `examples/gallery/`: the reference's example app (its demos and diagnostics), tested headless and in the macOS host; differences in its `KNOWN_ISSUES.md`, licences in its `LICENSES.md`.
- `docs/`: [architecture.md](docs/architecture.md), [parity.md](docs/parity.md) and `parity/`, [guide/](docs/guide/README.md), `evidence/`.

## Checks

`.github/workflows/ci.yml` runs on every push to `master` and on pull requests:

- **format**: `cargo fmt --all -- --check`.
- **rust-macos**: `cargo clippy --workspace --all-targets -- -D warnings`, then `cargo test --workspace` on Metal.
- **rust-linux**: the same on Ubuntu 24.04 with GTK 4 and Mesa's software Vulkan (lavapipe) and GL (llvmpipe).
- **rust-windows**: Clippy and `cargo build --workspace --all-targets`; no tests.
- **cross**: Clippy for `wasm32-unknown-unknown`, `aarch64-linux-android` (with the runner's NDK) and `aarch64-apple-ios-sim`.
- **typescript**, on macOS with the NSH nightly of `f577c51` (its release asset checked by SHA-256, or the CLI built from that commit once the nightly has moved): `nsh check`, `nsh build` (a warning fails the job), `nsh test` in `examples/headless`.

None of these jobs has run yet (the repository is not published). The same commands pass locally on macOS (`docs/evidence/fixes-audit/`); whether lavapipe and the macOS runner's GPU are enough for the pixel tests is untested.

Locally, `just check`, `just test` and `just test-tsx` run the same gates with the repository's nightly (`.nsh-nightly/`); see [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

MIT, © 2026 hmziqrs. See [LICENSE](LICENSE).

nsh-webgpu ports react-native-webgpu (MIT, William Candillon) and vendors @webgpu/types (BSD 3-Clause); the gallery adapts webgpu-samples (BSD 3-Clause) and ships third-party assets. [NOTICE.md](NOTICE.md) carries each licence and what derives from it, and `examples/gallery/LICENSES.md` lists the gallery's files by origin. Two items are open before publishing: webgpu-samples' licence text is to be pasted into NOTICE.md, and the Stanford dragon mesh's terms are not established.

Repository: https://github.com/SuperNSH/webgpu
