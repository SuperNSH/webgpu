# GPUDevice extensions

react-native-webgpu adds to `GPUDevice`, `GPUQueue` and the WebGPU globals. Each addition and its state here:

| Addition | State |
|---|---|
| `device.importSharedTextureMemory(descriptor)` | not exported. Its descriptor type `GPUSharedTextureMemoryDescriptor` is. wgpu 29 has no shared texture memory |
| `device.importSharedFence(descriptor)` | not exported; `GPUSharedFenceDescriptor` and the fence types are. Same reason |
| Importing a buffer once | not applicable without the two above |
| `requestDevice({ dawnToggles })` | accepted, ignored: Dawn-only |
| `requestDevice({ implicitDeviceSynchronization })` | accepted, ignored: wgpu's devices are always safe to use from several threads |
| `GPUExternalTexture` extensions | partly, below |
| `copyExternalImageToTexture` with a native frame | `ERR_WEBGPU_UNSUPPORTED` |
| `createImageBitmap` | implemented, below |
| `getPreferredHighBitDepthCanvasFormat()` | implemented |

The missing methods are a gap against the reference's export list: by the project's rule they should exist and throw `ERR_WEBGPU_UNSUPPORTED`. Until then, `device.importSharedTextureMemory` is `undefined`.

## GPUExternalTexture

`device.importExternalTexture({ source })` takes a `GPUTexture` (`rgba8unorm` or `bgra8unorm`, with `TEXTURE_BINDING`) and makes a wgpu 29 external texture with one RGBA plane. The reference's extras are applied: `cropOrigin` / `cropSize` (outside the texture throws, as the reference), `mirrored`, `rotation` (0, 90, 180, 270; anything else is a `TypeError`) and `colorSpace: "display-p3"`. `destroy()` is idempotent. Tested against the reference's snapshots in `tests/commands/external.rs`.

It needs wgpu's `EXTERNAL_TEXTURE` feature, which `requestDevice` turns on when the adapter has it; without it the call throws `ERR_WEBGPU_UNSUPPORTED`. A `NativeBuffer`, `HTMLVideoElement` or `VideoFrame` source throws `ERR_WEBGPU_UNSUPPORTED`.

## copyExternalImageToTexture

Sources: `ImageBitmap` and `ImageData` (`{ data, width, height }`), with `origin` and `flipY`. The destination's `premultipliedAlpha`, `colorSpace` (`"srgb"`, `"display-p3"`), `mipLevel`, `origin` and `aspect` are honoured, for every format the spec lists, with the reference's alpha rounding bit for bit. The destination needs `RENDER_ATTACHMENT | COPY_DST`, as the spec says (the reference does not check). A native frame (`GPUCopyExternalNativeFrameSourceInfo`), a canvas or a DOM element throws `ERR_WEBGPU_UNSUPPORTED`.

## createImageBitmap

`createImageBitmap(image, options?)` and `createImageBitmap(image, sx, sy, sw, sh, options?)`. Sources: an `ArrayBuffer` or view of PNG, JPEG or WebP bytes, `ImageData`, `ImageBitmap`, or a Blob-like object with `arrayBuffer()`. Options: `premultiplyAlpha`, the crop rectangle, `resizeWidth` / `resizeHeight` (bilinear; `resizeQuality: "pixelated"` is nearest), `imageOrientation: "flipY"`. `colorSpaceConversion` is ignored and EXIF orientation is not read. Bytes that do not decode reject with `InvalidStateError`; a look-alike buffer with `TypeError`. The [Platform guide](../getting-started/native-api.md#loading-images) has a sample.

## getPreferredHighBitDepthCanvasFormat

Returns `"rgb10a2unorm"` on Android and `"rgba16float"` everywhere else (NSH's `platform.os`). See [Platform guide, High bit depth](../getting-started/native-api.md#high-bit-depth).

`docs/parity/commands.md` and `docs/parity/react-parity.md` list every case with its test.
