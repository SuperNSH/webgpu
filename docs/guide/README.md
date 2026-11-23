# nsh-webgpu guide

The NSH counterparts of react-native-webgpu's docs (`apps/docs/content` at `06599f9`). Each page says where NSH differs from React Native and what nsh-webgpu does not do. [docs/parity.md](../parity.md) and `docs/parity/*.md` hold the full list, export by export.

Code samples come from code that ran. Each one names its source: an example app, a test, or the samples in `examples/headless/samples/`, which `nsh check`, `nsh build` and `nsh test` cover in `examples/headless` (`docs/evidence/fixes-audit/gates-tsx.txt`; the README's example is mounted by `samples.test.tsx`). A sample marked "untested" was not run or type-checked.

## Getting started

| Reference page | Here |
|---|---|
| `getting-started/installation` | [Installation](getting-started/installation.md) |
| `getting-started/expo` | none: NSH has no Expo. Installation covers creating an app that uses the package |
| `getting-started/canvas` | [Canvas](getting-started/canvas.md) |
| `getting-started/react-api` | [NSH API](getting-started/nsh-api.md): the hooks and the provider on NSH signals |
| `getting-started/native-api` | [Platform guide](getting-started/native-api.md) |
| `getting-started/offscreen-canvas` | [Offscreen canvas](getting-started/offscreen-canvas.md) |
| `getting-started/native-buffers` | [NativeBuffer](api/native-buffer.md): unsupported |
| `getting-started/learn-webgpu` | [Learn WebGPU](getting-started/learn-webgpu.md) |

## API

| Reference page | Here |
|---|---|
| `api/index` | [API overview](api/index.md) |
| `api/canvas` | [Canvas](api/canvas.md) |
| `api/gpu-device-provider` | [GPUDeviceProvider](api/gpu-device-provider.md) |
| `api/warn-if-not-hardware-accelerated` | [warnIfNotHardwareAccelerated](api/warn-if-not-hardware-accelerated.md) |
| `api/request-animation-frame` | [requestAnimationFrame](api/request-animation-frame.md) |
| `api/install-webgpu` | [installWebGPU](api/install-webgpu.md) |
| `api/gpu-device-extensions` | [GPUDevice extensions](api/gpu-device-extensions.md) |
| `api/native-buffer` | [NativeBuffer](api/native-buffer.md) |
| `api/video-player` | [Video player](api/video-player.md) |
| `api/draw-element-image-to-texture` | [drawElementImageToTexture](api/draw-element-image-to-texture.md) |

## Integrations

[Integrations](integrations.md): three.js, React Three Fiber, TypeGPU, TensorFlow.js, React Native Skia, VisionCamera and worklets. None is supported or tested.
