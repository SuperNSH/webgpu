# NativeBuffer

Unsupported, except `createTestVideoFrame` and `useNativeBufferSize`.

In react-native-webgpu a `NativeBuffer` is a platform image buffer (an `IOSurface` / `CVPixelBuffer` on Apple platforms, an `AHardwareBuffer` on Android) that WebGPU reads or renders into without a copy, through Dawn's shared texture memory, and that `NativeBufferView` shows on screen without a swap chain. wgpu 29 has no shared texture memory and no way to wrap such a buffer as a texture, and NSH has no native view that displays one. The exports keep their names and types:

| Export | Here |
|---|---|
| `NativeBuffer` (type) | as the reference: `handle`, `width`, `height`, `pixelFormat` (`"bgra8"` or `"nv12"`), `acquireFenceFd`, `release()` |
| `createNativeBuffer(width, height, options?)` | throws `ERR_WEBGPU_UNSUPPORTED` |
| `createNativeBufferFromPointer(pointer)` | throws `ERR_WEBGPU_UNSUPPORTED` |
| `NativeBufferView` | throws `ERR_WEBGPU_UNSUPPORTED` when rendered; its props and ref types (`NativeBufferViewProps`, `NativeBufferViewRef`, `NativeBufferResizeMode`, `NativeBufferPixelSize`) match the reference |
| `useNativeBufferSize(debounceMs = 150)` | implemented: the first size at once, later ones after `debounceMs` of stability. Only useful with something that reports pixel sizes |
| `createTestVideoFrame(width, height)` | implemented, in memory (bgra8) |

## createTestVideoFrame

Makes the reference's bgra8 test pattern byte for byte: a red/green gradient with diagonal blue stripes. Its signature is the reference export's, `(width, height) => NativeBuffer`. The frame lives in memory, so `handle` is `0n` and `acquireFenceFd` is `null`. The object also has `plane(index)`, the bytes of a plane, for `queue.writeTexture()`; it is not on the `NativeBuffer` type, so reach it with a cast:

```ts
const frame = createTestVideoFrame(SIZE, 3) as NativeBuffer & { plane(index?: number): Uint8Array };
const source = device.createTexture({
  size: [SIZE, 3],
  format: "bgra8unorm",
  usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
});
device.queue.writeTexture({ texture: source }, frame.plane(), { bytesPerRow: SIZE * 4 }, [SIZE, 3]);
frame.release();
```

(From `offscreenFrame` in `examples/headless/react-parity.app.tsx` on the integrated tree, which then draws `source` into a `GPUOffscreenCanvas` and checks the read-back byte for byte; its broken-on-purpose run is `docs/evidence/integration/broken-on-purpose-offscreen-frame-copy.txt`.) Write into a texture of your own: a `GPUOffscreenCanvas` texture has no `COPY_DST`, as in the reference, so `writeTexture` into it is a validation error. The reference's nv12 frame (BT.709 limited range, red left, green right) comes only from its `RNWebGPU.createTestVideoFrame(width, height, "nv12")` global, which nsh-webgpu does not install; the same frame is made by the package's internal `testVideoFrame` (`tsx/video.ts`), tested in `examples/headless/react-parity.test.tsx`.

The reference's [Native Buffers guide](https://github.com/wcandillon/react-native-webgpu) (flow, fences, lifetimes, platform notes) has no counterpart here.
