# Installation

nsh-webgpu brings the [WebGPU API](https://www.w3.org/TR/webgpu/) to NSH apps on [wgpu](https://wgpu.rs) 29, the GPU library NSH itself draws with. It is an nshm: a Rust crate (`nsh-webgpu/native`) and a TSX package (`nsh-webgpu`) in one directory, the repository's root.

## Requirements

| | Needed |
|---|---|
| NSH | the nightly built from `2964346` (`nsh 0.6.0 (2964346 nightly 2026-11-24)`). The 0.6.0 release does not work: the crate takes NSH from Git at that commit |
| Rust | stable; the workspace declares `rust-version = "1.97"`. Every app using the package builds Rust |
| App | any TSX app; a crate of its own only with a CLI built from source (below) |

The backend per platform is the one wgpu picks there:

| Platform | Backend | State |
|---|---|---|
| macOS | Metal | tested (cargo, `nsh test`), on screen in CI |
| iOS | Metal | on screen in the simulator; no physical device |
| Android | Vulkan, else GLES (one backend per instance) | the merged code on screen in an emulator, on GLES (`examples/canvas-native`, `examples/canvas-apple`); no physical device |
| Web | the browser's WebGPU, not wgpu | on screen in Chrome |
| Windows | DX12, Vulkan, GL | on screen in CI, DX12 on WARP (software) |
| Linux | Vulkan, GL | on screen in CI, Vulkan on lavapipe (software) |

The evidence for each row is in the README's "Status".

## Install NSH

```sh
curl -fsSL https://nsh.dev/install.sh | sh -s -- --nightly
nsh --version
```

The nightly release is rebuilt from `main` every night it changes and keeps no older builds. If `nsh --version` names another commit than `2964346`, build the CLI from that commit instead: clone `https://github.com/longbridge/nsh`, check out `2964346c285f57004f2531fb768cc9840b8d7ff5`, run `cargo build --release --locked -p nsh-cli`, and use `target/release/nsh`. A CLI built in a checkout builds its hosts from that checkout. CI does the same when the nightly has moved (`.github/workflows/ci.yml`, job `typescript`).

## Add the package

Create a TSX app with `nsh init my-app --template ts`, or use an existing one. Then, in its directory:

```sh
nsh add github:SuperNSH/webgpu
```

or, from a clone:

```sh
git clone https://github.com/SuperNSH/webgpu ../webgpu
nsh add --path ../webgpu
```

`nsh add` writes the dependency into `nsh.json` and pins it in `nsh.lock`. nsh reads an nshm at the root of what it gets, a directory or a Git commit unpacked whole, and the repository's root is the `nsh-webgpu` crate with its `nsh.json` (`docs/evidence/fixes/nsh-add-root.txt`: `nsh add --path` on the root). Both forms were run on an app with no crate (`nsh check`, `build`, `test`; `docs/evidence/platforms/runs.txt`).

## A crate of the app's own (CLI built from source)

With the nightly's own CLI the app needs no crate: nsh builds it against the nightly's commit ([longbridge/nsh#2980](https://github.com/longbridge/nsh/pull/2980)), the one nsh-webgpu is pinned to. A CLI built from a checkout takes NSH from that checkout instead, and the app then needs a crate pinned to `2964346`, so that nsh takes every NSH crate from there ([docs/architecture.md](../../architecture.md), "Blockers"). Copy `examples/headless/native` and rename it:

`nsh.json`:

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

`native/Cargo.toml`:

```toml
[package]
name = "my-app-native"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
nsh-core = { git = "https://github.com/longbridge/nsh", rev = "2964346c285f57004f2531fb768cc9840b8d7ff5" }

[workspace]
```

`native/src/lib.rs`:

```rust
use nsh_core::prelude::*;

#[native_module("my-app/native")]
fn native(_registry: &mut Registry) {}
```

Keep `examples/headless/native/Cargo.lock` beside it. It was seeded from NSH's own lock and moves with the pin; newer patch releases of NSH's dependencies have broken the build before (`rolldown_std_utils` 1.3.3).

The samples app in `examples/headless/samples/` is set up this way and passes `nsh check`, `nsh build` and `nsh test`.

## Windows: the shader compiler

wgpu compiles shaders for DX12 with the system's FXC unless it finds DXC, and FXC refuses some valid WGSL (a write to an array element by a dynamic index: the gallery's Wireframe). Turn DXC on with the crate's feature, on x86_64 MSVC:

```sh
nsh add github:SuperNSH/webgpu --features static-dxc
```

The build then downloads wgpu's static DXC library and checks its SHA-256. Or ship Microsoft's `dxcompiler.dll` (1.8.2502 or newer) with the app: wgpu finds it on the `PATH`.

## Setup

Importing `nsh-webgpu` installs `navigator.gpu`, the `GPU*` classes and the flag constants (`GPUBufferUsage`, ...) as globals, as react-native-webgpu's import does:

```tsx
import "nsh-webgpu";

const adapter = await navigator.gpu.requestAdapter();
const device = await adapter!.requestDevice();
```

(From `examples/headless/triangle.ts`.) Rendering on screen happens in a [`<Canvas>`](canvas.md): get a context from the canvas's ref, configure it with the device, encode passes, `device.queue.submit(...)`, then `context.present()`.

## Testing

There is no mock: `nsh test` runs the tests with the real package, and the headless runner nsh builds with the crate reaches the GPU (Metal on a Mac). A `<Canvas>` in a headless test has no native view, so its context renders into an offscreen texture you can copy out and check (`examples/headless/surfaces-apple.app.tsx`). `cargo test` covers the Rust module the same way (`tests/`).

On a machine without a GPU, wgpu may offer a software adapter (lavapipe, WARP) or none. `requestAdapter()` resolves `null` when there is none.

## Troubleshooting

- **`no [package] name` from `nsh add`**: the path is not the repository's root (an older checkout kept the crate in `crates/nsh-webgpu`; pass that directory there).
- **E0308 `expected Registry, found ...Registry` while building hosts**: the CLI is not the nightly `2964346`, and the app has no crate of its own pinned to it. See above.
- **Android emulators**: wgpu found no Vulkan adapter on the emulator used here and ran on GLES. `warnIfNotHardwareAccelerated` warns when the adapter is a software fallback.
- **`nsh run --target android` hangs in `zstd-sys`'s build script**: seen with NDK 29.0.14206865's `llvm-ar` on one Mac; `ANDROID_NDK_HOME` pointing at NDK 28.2 worked (`docs/parity/surfaces-other.md`).
