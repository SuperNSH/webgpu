# Integrations

react-native-webgpu documents seven libraries on top of it. None of them is supported by nsh-webgpu, and none has been tried. Some cannot work on NSH at all; others might, and are marked untested.

| Integration | State | Why |
|---|---|---|
| Worklets | not applicable | NSH runs TSX on one JS thread and has no worklet runtimes. `installWebGPU()` keeps its name ([installWebGPU](api/install-webgpu.md)). |
| React Three Fiber | unsupported | It is a React renderer. NSH has no React: its components run once and update through signals. |
| React Native Skia | unsupported | It shares a Dawn device and textures with react-native-webgpu (`importDevice`, `adoptTexture`, shared texture memory). nsh-webgpu runs wgpu, which cannot adopt Dawn objects, and Skia does not run on NSH. |
| VisionCamera | unsupported | A React Native camera library. Its frames reach WebGPU as `NativeBuffer`s through `importExternalTexture`, which needs shared texture memory that wgpu 29 lacks ([NativeBuffer](api/native-buffer.md)). |
| three.js (`three/webgpu`) | untested | Its `WebGPURenderer` needs `navigator.gpu` and a canvas whose `getContext("webgpu")` works, which nsh-webgpu provides, plus a `present()` after each frame. The reference also needs an entry-point alias, static class blocks and a `TextDecoder` polyfill under Metro. Nobody has bundled it with NSH, and its DOM use (canvas elements, image loading) is unchecked. |
| TensorFlow.js (`@tensorflow/tfjs-backend-webgpu`) | untested | It needs only a device, no canvas, so it may run; the reference adds a platform shim for React Native. Not tried. |
| TypeGPU | untested | It is written against `navigator.gpu` and the spec's objects, so `tgpu.init()` may work unchanged. Its `"use gpu"` functions need the `unplugin-typegpu` build plugin, and NSH's bundler takes no Babel or unplugin plugins that we know of; without it those functions are not turned into WGSL. WGSL written as strings does not need it. Not tried. |

## Using an npm library

NSH needs no `node_modules`, but its bundler resolves bare imports from one if you install packages with a package manager, and takes their browser builds. There is no DOM, so a package that touches `document`, `HTMLCanvasElement` or `Image` fails. A library that only calls `navigator.gpu` and the `GPU*` objects is the most likely to work, with two NSH rules on top: call `context.present()` after each `submit()` ([Canvas](getting-started/canvas.md#frame-presentation)), and size the canvas with `pixelRatio()`.

If you get one of the libraries above running, the evidence (the commands and what ran) belongs in a pull request that adds it to this page.
