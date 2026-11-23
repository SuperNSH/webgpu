# warnIfNotHardwareAccelerated

`warnIfNotHardwareAccelerated(adapter)` prints a warning with `console.warn` when `adapter.info.isFallbackAdapter` is true, with the reference's text:

```text
GPUAdapter is not hardware accelerated. This is common on Android emulators. Rendering will be slow. Some features may be unavailable.
```

`useDevice` and `GPUDeviceProvider` call it after `requestAdapter()`. Call it yourself when you request the adapter directly:

```ts
import { warnIfNotHardwareAccelerated } from "nsh-webgpu";

export async function adapterChecked(): Promise<GPUAdapter> {
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("No appropriate GPUAdapter found.");
  warnIfNotHardwareAccelerated(adapter);
  return adapter;
}
```

(`examples/headless/samples/errors.ts`, type-checked. `react-parity.test.tsx` checks the warning for a fallback adapter and the silence otherwise.)

`isFallbackAdapter` is true when the request asked for `forceFallbackAdapter`, or when wgpu reports the adapter's device type as `Cpu` (lavapipe, llvmpipe, WARP, SwiftShader).
