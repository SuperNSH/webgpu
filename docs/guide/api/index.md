# API overview

nsh-webgpu implements the [WebGPU API](https://www.w3.org/TR/webgpu/). `GPUDevice`, `GPUBuffer`, `GPUTexture`, `GPURenderPipeline` and the rest are TSX classes that `implements` the interfaces of `@webgpu/types` 0.1.74 (vendored in `tsx/types/webgpu.d.ts`), so `nsh check` fails on any drift from the spec's shapes. They are also globals, and `instanceof` works against both. Code written for the spec runs unchanged, within the differences below.

To learn the API: [WebGPU Fundamentals](https://webgpufundamentals.org/), the [MDN WebGPU reference](https://developer.mozilla.org/en-US/docs/Web/API/WebGPU_API), the [specification](https://www.w3.org/TR/webgpu/).

## Where it differs from the spec

wgpu, not Dawn, is underneath, and some of the spec's surface has no wgpu equivalent. The main differences (the full list is [docs/parity.md](../../parity.md) and `docs/parity/*.md`):

- `getMappedRange()` returns a copy; writes reach the buffer at `unmap()`.
- `GPUAdapterInfo.architecture` is `""`; `vendor` comes from the PCI id.
- `GPUSupportedFeatures` lists only spec-named features. `texture-formats-tier2`, `texture-component-swizzle`, `subgroup-size-control`, `texture-compression-unaligned` and `atomic-vec2u-min-max` are never reported, and requiring one is a `TypeError`.
- `maxBindGroupsPlusVertexBuffers` is `maxBindGroups + maxVertexBuffers`; the per-stage storage limits are wgpu's per-shader-stage ones; `maxAnisotropy` is clamped to 16.
- `requestAdapter({ featureLevel })` is accepted and ignored: a wgpu device is a core device.
- A `null` hole before a used slot in a pipeline's `vertex.buffers` reads the wrong buffer, a wgpu-core 29 bug (`docs/parity/commands.md`).
- `copyExternalImageToTexture` and `importExternalTexture` take no DOM sources (there is no DOM).
- Exceptions cross from Rust as `TypeError`, `RangeError` or a `DOMException` with the spec's name; a wgpu panic becomes an `OperationError` instead of crashing.

## What this section covers

The additions react-native-webgpu makes beyond the spec, and their state here:

| Page | State |
|---|---|
| [Canvas](canvas.md) | implemented |
| [GPUDeviceProvider](gpu-device-provider.md) | implemented |
| [warnIfNotHardwareAccelerated](warn-if-not-hardware-accelerated.md) | implemented |
| [requestAnimationFrame](request-animation-frame.md) | NSH's own |
| [installWebGPU](install-webgpu.md) | implemented; no worklet runtimes |
| [GPUDevice extensions](gpu-device-extensions.md) | partly: `createImageBitmap`, external textures from a `GPUTexture`, the high-bit-depth format; Dawn toggles ignored; shared texture memory and fences missing |
| [NativeBuffer](native-buffer.md) | unsupported, except `createTestVideoFrame` |
| [Video player](video-player.md) | unsupported |
| [drawElementImageToTexture](draw-element-image-to-texture.md) | not exported |

The guides cover [`GPUOffscreenCanvas`](../getting-started/offscreen-canvas.md) and [`importDevice` / `adoptTexture`](../getting-started/native-api.md#importing-an-external-device).

## ERR_WEBGPU_UNSUPPORTED

What nsh-webgpu cannot do keeps its name and throws (or rejects with) an `Error` whose `code` is `"ERR_WEBGPU_UNSUPPORTED"` and whose message starts with `ERR_WEBGPU_UNSUPPORTED:` and says why:

```ts
try {
  return importDevice(pointer);
} catch (error) {
  if ((error as { code?: string }).code === "ERR_WEBGPU_UNSUPPORTED") return null;
  throw error;
}
```

(`examples/headless/samples/errors.ts`, type-checked.)
