# Installation

nsh-webgpu brings the [WebGPU API](https://www.w3.org/TR/webgpu/) to NSH apps on [wgpu](https://wgpu.rs) 29, the GPU library NSH itself draws with. It is an nshm: a Rust crate (`nsh-webgpu/native`) and a TSX package (`nsh-webgpu`) in one directory, the repository's root.

## Requirements

| | Needed |
|---|---|
| NSH | 0.7 |
| Rust | stable; the workspace declares `rust-version = "1.97"`. Every app using the package builds Rust |
| App | any TSX app; no Rust crate of its own |

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
curl -fsSL https://nsh.dev/install.sh | sh
nsh --version   # nsh 0.7.1 or a later 0.7
```

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
- **E0308 `expected Registry, found ...Registry` while building hosts**: the CLI and nsh-webgpu use two versions of `nsh-core`. Use an NSH 0.7 CLI.
- **Android emulators**: wgpu found no Vulkan adapter on the emulator used here and ran on GLES. `warnIfNotHardwareAccelerated` warns when the adapter is a software fallback.
- **`nsh run --target android` hangs in `zstd-sys`'s build script**: seen with NDK 29.0.14206865's `llvm-ar` on one Mac; `ANDROID_NDK_HOME` pointing at NDK 28.2 worked (`docs/parity/surfaces-other.md`).
