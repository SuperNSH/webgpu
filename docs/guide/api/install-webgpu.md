# installWebGPU

`installWebGPU()` puts `navigator.gpu`, every `GPU*` class, the error classes and the flag constants (`GPUBufferUsage`, `GPUTextureUsage`, `GPUShaderStage`, `GPUMapMode`, `GPUColorWrite`) on `globalThis`, and `createImageBitmap` when the runtime has none. It is idempotent.

Importing `nsh-webgpu` calls it, as react-native-webgpu's import installs WebGPU. You do not need to call it yourself:

```ts
import "nsh-webgpu";

const adapter = await navigator.gpu.requestAdapter();
```

`WebGPUModule.install()` calls it too and returns `true`, for code written against the reference's TurboModule.

## Worklets

The reference's `installWebGPU` is a worklet: you call it on a UI-thread or dedicated worklet runtime so WebGPU objects work there. NSH runs TSX on one JS thread and has no worklet runtimes, so there is no other runtime to install on. The function keeps its name and works on the one runtime there is.

## Web

On the web `installWebGPU()` does nothing: `navigator.gpu` and the classes stay the browser's, as in the reference's web build. `examples/headless/integration.test.tsx` checks both sides, opening the app as the web and as a native platform; its broken-on-purpose run is `docs/evidence/integration/broken-on-purpose-install-web-noop.txt`.
