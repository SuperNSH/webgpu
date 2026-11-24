# nsh-webgpu

WebGPU for Native Shell (NSH) apps: a port of [react-native-webgpu](https://github.com/wcandillon/react-native-webgpu) on [wgpu](https://wgpu.rs) 29.

The same JavaScript API as the reference: `navigator.gpu` and the spec's `GPU*` classes, `<Canvas>` with `getContext("webgpu")` and `present()`, the hooks (`useDevice`, `useCanvasRef`, `useSurface`, `GPUDeviceProvider`, `useMainDevice`), `GPUOffscreenCanvas`, `requestAnimationFrame`, `createImageBitmap`. Where react-native-webgpu runs Dawn, nsh-webgpu runs wgpu, which NSH already uses. Exports wgpu cannot back (Dawn device import, platform buffers, video) keep their names and throw an error with `code: "ERR_WEBGPU_UNSUPPORTED"`; [docs/parity.md](docs/parity.md) lists each with the reason.

Part of the SuperNSH capability set. Guides in [docs/guide/](docs/guide/README.md), CI in `.github/workflows/ci.yml`.

## Status

**0.1.0, on GitHub (not on crates.io). `<Canvas>` runs on screen on macOS, the iOS simulator, an Android emulator, Windows, Linux and the web.** It needs the NSH nightly `2964346`, not the 0.6.0 release.

| Platform | GPU it ran on | What was checked | Evidence |
|---|---|---|---|
| macOS | Apple Silicon (Metal) | `cargo test`; `nsh test` in `examples/headless` (89), `examples/canvas-apple` (1) and `examples/gallery` (45); the Triangle and the rotating Cube, and the whole gallery (44 tests), in a window, in CI | `docs/evidence/platforms/macos-canvas.png`, `gallery-macos.jpg` |
| iOS 27 simulator | Metal | the Triangle and the rotating Cube on screen | `docs/evidence/platforms/ios-canvas.png` |
| Android emulator | GLES | the same | `docs/evidence/platforms/android-canvas.png` |
| Windows | DX12 on WARP (software) | `cargo test`; the same in a window, in CI, with DXC (`static-dxc`) | `docs/evidence/platforms/windows-canvas.png`, `gallery-windows.jpg` |
| Linux | Vulkan on lavapipe (software) | `cargo test`, in CI and in an arm64 container; the same in a window, in CI (not Import External Texture: wgpu 29 has none on Vulkan) | `docs/evidence/platforms/linux-canvas.png`, `gallery-linux.jpg` |
| Web | the browser's WebGPU | the README's example as an app that uses the package, in Chromium | `docs/evidence/platforms/web-readme-app.jpg` |

The desktops are checked by `.github/workflows/onscreen.yml` on every push: it runs `examples/canvas-apple`, takes two screenshots and looks for the triangle and for a cube that moved (`scripts/onscreen.py`), then runs the gallery's tests in the platform's own host and takes a picture of each page (`nsh test --target`, `examples/gallery/tour.test.tsx`).

Not verified: a hardware GPU on Windows, Linux or Android, a physical iPhone or Android device, and Mac Catalyst.

On Android the canvases clear to black rather than showing the views under them: the emulator's GL surfaces offer only opaque compositing, so `alphaMode: "premultiplied"` falls back to opaque there (`docs/architecture.md`, "The contract"). The earlier screenshot `docs/evidence/integration/android-canvas-apple.png` also shows an empty Triangle canvas: the example's capture copied out of the surface texture, which Android's GL surfaces cannot do (no `COPY_SRC`), so that frame's submit was invalid; the example now skips the capture on Android, and `docs/evidence/fixes/android-canvas-apple.png` shows the triangle (the explanation is in `docs/evidence/fixes/android-canvas-apple.txt`).

The gallery (`examples/gallery`) ports the reference's example app: its demos and diagnostics, with the differences listed in `examples/gallery/KNOWN_ISSUES.md`; its tests pass headless and in the macOS, Windows and Linux hosts.

With that nightly an app needs no Rust crate of its own: `nsh` builds an app whose only Rust is an nshm against the nightly's own commit ([longbridge/nsh#2980](https://github.com/longbridge/nsh/pull/2980)), the commit nsh-webgpu is pinned to. The CLI and the pin have to be the same commit.

The crate is the repository's root (its `Cargo.toml` and `nsh.json`), which is where `nsh add` reads an nshm, from a path or from Git: `nsh add github:SuperNSH/webgpu --rev master` installs it.

## Install

You need Rust and the NSH nightly built from `2964346` (`nsh 0.6.0 (2964346 nightly 2026-11-24)`). The nightly release on GitHub is rebuilt from `main` every night, so it only matches while `main` has not moved: check `nsh --version`, or build the CLI from that commit (`cargo build --release --locked -p nsh-cli` in a checkout of `longbridge/nsh` at `2964346`).

```sh
curl -fsSL https://nsh.dev/install.sh | sh -s -- --nightly
nsh --version   # nsh 0.6.0 (2964346 nightly ...)
```

Add the package, in the app's directory, from GitHub or from a clone:

```sh
nsh add github:SuperNSH/webgpu
# or
git clone https://github.com/SuperNSH/webgpu ../webgpu
nsh add --path ../webgpu
```

That is all: import from `nsh-webgpu` and `nsh run`.

**With a CLI built from source** (once the nightly has moved), nsh takes its crates from that checkout, so the app needs a crate of its own pinned to the commit, for both to share one `nsh-core`: copy `examples/headless/native` (its `Cargo.lock` too) into the app as `native/`, and add `"rust": "native"` to `nsh.json` ([installation guide](docs/guide/getting-started/installation.md)). The examples here and CI are set up that way.

On the web NSH builds the app's crate for wasm, and one of NSH's dependencies (zstd) is C: `llvm-ar` and a clang that targets wasm must be found (macOS: `brew install llvm`, then `PATH="/opt/homebrew/opt/llvm/bin:$PATH"` and `CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang`). On Linux the development host also needs GStreamer (`libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev`), beside GTK 4 and WebKitGTK; `.github/workflows/onscreen.yml` has the whole list.

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

- The root is the nshm: `Cargo.toml` (the `nsh-webgpu` package and its workspace; NSH crates from Git at `2964346`, `wgpu` 29) and `nsh.json` (its TSX exports). `src/` is the Rust module `nsh-webgpu/native` (one `#[module]` per spec interface, the handle registry, the canvas views per platform in `src/surface/`); `tsx/` is the TSX package `nsh-webgpu` (the spec classes in `tsx/api/`, react-native-webgpu's own exports beside them, the vendored `@webgpu/types` in `tsx/types/`); `tests/` holds the cargo tests.
- `examples/headless/`: the TSX tests (`nsh test`), using the package as an app does.
- `examples/canvas-apple/`, `examples/canvas-native/`, `examples/canvas-web/`: the canvas on screen on Apple platforms, Android/Windows/Linux, and the web.
- `examples/gallery/`: the reference's example app (its demos and diagnostics), tested headless and in the macOS host; differences in its `KNOWN_ISSUES.md`, licences in its `LICENSES.md`.
- `docs/`: [architecture.md](docs/architecture.md), [parity.md](docs/parity.md) and `parity/`, [guide/](docs/guide/README.md), `evidence/`.

## Checks

`.github/workflows/ci.yml` runs on every push to `master` and on pull requests:

- **format**: `cargo fmt --all -- --check`.
- **rust-macos**: `cargo clippy --workspace --all-targets -- -D warnings`, then `cargo test --workspace` on Metal.
- **rust-linux**: the same on Ubuntu 24.04 with GTK 4 and Mesa's software Vulkan (lavapipe) and GL (llvmpipe).
- **rust-windows**: Clippy and the tests, DX12 on WARP.
- **cross**: Clippy for `wasm32-unknown-unknown`, `aarch64-linux-android` (with the runner's NDK) and `aarch64-apple-ios-sim`.
- **typescript**, on macOS with the NSH nightly of `2964346` (its release asset checked by SHA-256, or the CLI built from that commit once the nightly has moved): `nsh check`, `nsh build` (a warning fails the job), `nsh test` in `examples/headless`.

`.github/workflows/onscreen.yml` runs `<Canvas>` in a window on macOS, Linux and Windows (see Status).

Locally, `just check`, `just test` and `just test-tsx` run the same gates with the repository's nightly (`.nsh-nightly/`); see [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

MIT, © 2026 hmziqrs. See [LICENSE](LICENSE).

nsh-webgpu ports react-native-webgpu (MIT, William Candillon) and vendors @webgpu/types (BSD 3-Clause); the gallery adapts webgpu-samples (BSD 3-Clause) and ships third-party assets. [NOTICE.md](NOTICE.md) carries each licence and what derives from it, and `examples/gallery/LICENSES.md` lists the gallery's files by origin.

Repository: https://github.com/SuperNSH/webgpu
