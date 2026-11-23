# Platform guide

nsh-webgpu implements the WebGPU API as a browser does, on wgpu. Where NSH or wgpu forces a difference, this page says so. The reference's page of the same name covers Dawn; here is what each of its sections means on nsh-webgpu.

## Threading model

WebGPU runs on NSH's one JS thread. NSH has no worklet runtimes, so there is no UI-thread or worklet rendering, and `installWebGPU()` has nothing to install elsewhere ([installWebGPU](../api/install-webgpu.md)).

Under the API:

- Every WebGPU object is a `u32` id into one process-wide registry in Rust; the TSX classes hold the id. wgpu's objects are `Send + Sync`, so the native views create surfaces on the UI thread and the JS thread configures and presents them.
- `requestAdapter` and `requestDevice` are async native functions; wgpu's waits run on NSH's blocking threads. `mapAsync` and `onSubmittedWorkDone` start on the JS thread, in call order, and are awaited off it.
- `getMappedRange()` returns a copy of the mapped bytes; for a write mapping, `unmap()` writes the copy back and detaches it. No JS object points into memory wgpu may unmap. The cost is one copy each way.
- `device.lost` and `uncapturederror` are delivered on the JS thread. An uncaptured error with no listener is printed with `console.warn`, as browsers do.

[docs/architecture.md](../../architecture.md), "Binding design", has the details.

## Choosing a backend

The process has one `wgpu::Instance` (`src/instance.rs`), with wgpu's default backends for the platform (Metal on Apple platforms; Vulkan and DX12 on Windows; Vulkan and GL on Linux), read with wgpu's `from_env`: `WGPU_BACKEND` and the other `WGPU_*` variables override them. On Android the instance takes one backend, Vulkan when it has an adapter and GL otherwise, because a surface made for both connects the window twice and GL's then fails.

## Background GPU work on iOS

react-native-webgpu holds work submitted while an iOS app is in the background (Dawn's `metal_suspend_submissions_in_background`). nsh-webgpu does not: wgpu has no such switch, and nothing here was tested in the background. Stop submitting when the app leaves the foreground. A loop on [`requestAnimationFrame`](../api/request-animation-frame.md) follows NSH's frame clock; whether it pauses in the background has not been checked.

## Loading images

`createImageBitmap` is installed as a global when the runtime has none. It takes PNG, JPEG or WebP bytes (an `ArrayBuffer`, a view of one, or an object with `arrayBuffer()`), `ImageData` or another `ImageBitmap`, decodes them off the JS thread with the `image` crate, and feeds `copyExternalImageToTexture`:

```ts
/** The global installWebGPU() puts there (react-native-webgpu's overloads). */
const createImageBitmap = (globalThis as unknown as {
  createImageBitmap: (image: unknown, ...rest: unknown[]) => Promise<ImageBitmap>;
}).createImageBitmap;

export async function upload(device: GPUDevice, png: Uint8Array): Promise<GPUTexture> {
  const bitmap = await createImageBitmap(png, { premultiplyAlpha: "none" });
  const texture = device.createTexture({
    size: [bitmap.width, bitmap.height],
    format: "rgba8unorm",
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
  });
  device.queue.copyExternalImageToTexture({ source: bitmap }, { texture }, [bitmap.width, bitmap.height]);
  bitmap.close();
  return texture;
}
```

(`examples/headless/samples/image.ts`, type-checked. The calls are those of `examples/headless/commands.test.tsx`, "createImageBitmap and copyExternalImageToTexture".) The destination needs `COPY_DST` and `RENDER_ATTACHMENT`, as the spec says. The cast is the one `commands.test.tsx` uses; where the package's `tsx/types/dom.d.ts` declares `createImageBitmap` (the integrated tree does), call the global directly. Get the bytes from a file (`fs/promises`) or `fetch`. Unlike the reference's native decoder, the crop rectangle, `resizeWidth`/`resizeHeight` and `imageOrientation: "flipY"` are applied; `colorSpaceConversion` is ignored and EXIF orientation is not read. See `docs/parity/commands.md`.

## High bit depth

```ts
import { getPreferredHighBitDepthCanvasFormat } from "nsh-webgpu";

// "rgb10a2unorm" on Android, "rgba16float" elsewhere
const format = getPreferredHighBitDepthCanvasFormat();
context.configure({ device, format, alphaMode: "opaque" });
```

The function is tested (it returns `"rgba16float"` in the headless runner); an `rgba16float` canvas on screen, and the Android branch, are not. (Sample untested.)

## Native textures

Not supported. The reference moves camera frames, video frames and native buffers to and from the GPU through Dawn's shared texture memory. wgpu 29 has no shared texture memory, so `createNativeBuffer`, `createNativeBufferFromPointer`, `createVideoPlayer` and `NativeBufferView` throw `ERR_WEBGPU_UNSUPPORTED` ([NativeBuffer](../api/native-buffer.md)). `device.importExternalTexture()` works with a `GPUTexture` as its source ([GPUDevice extensions](../api/gpu-device-extensions.md#gpuexternaltexture)).

## Pipeline cache

None. wgpu has a pipeline cache (`wgpu::PipelineCache`, Vulkan only), and nsh-webgpu does not use it: each launch compiles its shaders and pipelines again. `dawnToggles` such as `disable_blob_cache` are accepted and ignored.

## Importing an external device

`importDevice(pointer)` and `adoptTexture(pointer)` exist and throw `ERR_WEBGPU_UNSUPPORTED`: they wrap Dawn's `WGPUDevice*` and `WGPUTexture*`, and wgpu cannot adopt Dawn objects.

```ts
import { importDevice } from "nsh-webgpu";

export function tryImport(pointer: bigint): GPUDevice | null {
  try {
    return importDevice(pointer);
  } catch (error) {
    if ((error as { code?: string }).code === "ERR_WEBGPU_UNSUPPORTED") return null;
    throw error;
  }
}
```

(`samples/errors.ts`, type-checked; the throw is tested in `react-parity.test.tsx`.)

## Rendering to a native window

Not available. `RNWebGPU.MakeWebGPUCanvasContextFromNativeWindow` has no counterpart: nsh-webgpu has no `RNWebGPU` global, and no export makes a context from a caller's `CAMetalLayer *` or `ANativeWindow *`. It would be a `create_surface_unsafe` on that handle (`docs/parity/surfaces-apple.md`). `getNativeSurface()` still returns the canvas's own handle.

## See also

- [Canvas](canvas.md): presenting frames, pixel density, transparency
- [NSH API](nsh-api.md)
- [GPUDevice extensions](../api/gpu-device-extensions.md)
