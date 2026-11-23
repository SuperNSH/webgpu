# Parity: area (b) commands

`GPUCommandEncoder`, `GPUCommandBuffer`, `GPURenderPassEncoder`, `GPUComputePassEncoder`, `GPURenderBundleEncoder`,
`GPURenderBundle`, `GPUQueue`, `GPUExternalTexture`, `createImageBitmap` / `ImageBitmap`, against
react-native-webgpu 06599f9 on wgpu 29.0.4. Verified on macOS 27.2, Apple Silicon, Metal only (evidence:
`docs/evidence/commands/`). Built on master `fdcde29` (areas (a) and (e) merged).

> Area notes from phase 2. The status of every export is in [docs/parity.md](../parity.md), which is current;
> rows here that phase 3 (integration) changed are updated and marked "phase 3".

## Design: passes record, the encoder replays

A wgpu pass borrows its `CommandEncoder` mutably until it is dropped, and a `RenderBundleEncoder<'a>` borrows every
resource for `'a`; the JS API hands out pass objects the program calls whenever it likes. So:

- **A pass is a command list.** `beginRenderPass` / `beginComputePass` resolve the descriptor's ids into owned wgpu
  handles (`OwnedRenderPass`, `OwnedComputePass` in `command_encoder.rs`) and register a pass entry. Each call
  (`setPipeline`, `setBindGroup`, `draw`, ...) resolves its ids at once (an unknown id is a `TypeError`, as WebIDL
  conversion is) and appends a `RenderCommand` / `ComputeCommand` holding cloned (reference-counted) wgpu handles,
  so a JS object collected before `end()` changes nothing.
- **`end()` replays.** It begins the wgpu pass on the encoder, replays the list, and drops the pass, inside one
  native call. wgpu validates the replayed pass; its errors surface at `finish()`, where the spec reports them too.
- **The encoder keeps the spec's state machine** (`EncoderState`): `open`, `locked` while a pass records, `ended`
  after `finish()`. A call on a locked encoder makes it invalid (no error yet); a call on an ended encoder or ended
  pass is a validation error at once; `finish()` of an invalid encoder (a pass left open, a pass found invalid, a
  copy found invalid, a call while locked) reports the first problem as a validation error and returns an invalid
  `GPUCommandBuffer`, which `submit` rejects.
- **Bundles share the draw commands** (`DrawCommand`). `createRenderBundleEncoder` validates the descriptor at once
  (the spec's layout rules wgpu does not check, then a trial `create_render_bundle_encoder`); `finish()` replays the
  list into a `wgpu::RenderBundleEncoder` borrowing the list. wgpu *panics* on an invalid bundle encoder descriptor
  or bundle (`handle_error_fatal`), so both run under `catch_unwind`: the panic becomes the spec's validation error
  and an invalid bundle (`RenderBundleEntry.bundle: None`); executing one makes the pass invalid.
- **What wgpu would panic on is checked first**: buffer slices out of range (`setVertexBuffer`, `setIndexBuffer`),
  buffer copy alignment and range, `clearBuffer` alignment, `bytesPerRow % 256` for buffer-texture copies,
  `firstQuery + queryCount` overflow, indirect offsets: each makes the pass or encoder invalid with a message.

## Done

| Export | Status | Notes |
|---|---|---|
| `device.createCommandEncoder`, `GPUCommandEncoder.label`, `finish(descriptor)` | done | label on encoder and command buffer; finishing twice → validation error, invalid buffer |
| `beginRenderPass` | done | every member: `colorAttachments` (nulls keep indices; `view`, `depthSlice`, `resolveTarget`, `clearValue` seq or dict, default transparent, `loadOp`, `storeOp`), `depthStencilAttachment` (depth/stencil clear values, load/store ops, `depthReadOnly`, `stencilReadOnly`; ops required for an aspect the format has and is not read-only, absent otherwise; `depthClearValue` required to clear and in [0, 1]: the spec's rules, a validation error at `finish()`), `occlusionQuerySet`, `timestampWrites`, `label`; `maxDrawCount` accepted and ignored (wgpu has no draw-count limit) |
| `beginComputePass` | done | `label`, `timestampWrites` |
| query sets in passes | done | `occlusionQuerySet`, `timestampWrites`, `resolveQuerySet`, `beginOcclusionQuery` check the set: invalid, destroyed (`QuerySetEntry::destroyed()`), wrong type, `queryIndex >= count` → validation error (pass or encoder invalid). The check is at record time; a set destroyed between recording and `submit` is wgpu's to catch |
| `copyBufferToBuffer` | done | both overloads (`(src, dst, size?)` and with offsets); offsets/size `[EnforceRange]` (`-4` → `TypeError`, EnforceRange.spec); multiples of 4 and in range, else validation error at `finish()` |
| `copyBufferToTexture`, `copyTextureToBuffer`, `copyTextureToTexture` | done | `GPUTexelCopy*Info` with `origin` seq or dict, `mipLevel`, `aspect`; `bytesPerRow % 256` checked; the rest validated by wgpu |
| `clearBuffer` | done | `offset`, `size` (default: to the end), multiples of 4 |
| `resolveQuerySet` | done | `destinationOffset % 256` |
| `push/pop/insertDebug*` on encoder, render pass, compute pass | done | unbalanced groups → validation error at `finish()` |
| `writeTimestamp` | n/a | removed from the spec; the reference has none either |
| `setPipeline`, `setBindGroup` (render, compute, bundle) | done | `null` unbinds; dynamic offsets as a sequence or `Uint32Array` + `dynamicOffsetsDataStart` + `dynamicOffsetsDataLength` (past the end → `RangeError`); an invalid bind group (`BindGroupEntry.valid`) → invalid pass |
| `setVertexBuffer` | done | `offset` (multiple of 4) / `size` (default: to the end); out of range → invalid pass; `null`, or an empty range, unbinds: wgpu has no empty binding, so a 4-byte zeroed buffer stands in, and a pipeline that reads the slot fails wgpu's size check, as an unbound slot fails the spec's |
| `setIndexBuffer` | done | `uint16`/`uint32`, offset aligned to the index size; an empty range → invalid pass (wgpu has no empty slice) |
| `draw`, `drawIndexed`, `drawIndirect`, `drawIndexedIndirect` | done | defaults; `baseVertex` signed; indirect offset multiple of 4 with the arguments inside the buffer |
| `setViewport`, `setScissorRect`, `setBlendConstant`, `setStencilReference` | done | viewport depth range and sizes checked as the spec does |
| `beginOcclusionQuery`, `endOcclusionQuery` | done | see query sets |
| `executeBundles` | done | invalid bundles invalidate the pass |
| `setImmediates` (render, compute, bundle) | done | `dataOffset`/`size` in elements like `writeBuffer`; offset and size multiples of 4; needs wgpu's immediates support on the device |
| `dispatchWorkgroups`, `dispatchWorkgroupsIndirect` | done | defaults 1; indirect offset multiple of 4, 12 bytes inside |
| `device.createRenderBundleEncoder`, `GPURenderBundleEncoder`, `GPURenderBundle` | done | `colorFormats` (nulls kept), `depthStencilFormat`, `sampleCount`, `depthReadOnly`, `stencilReadOnly`; `instanceof GPURenderBundleEncoder` works (issue #564) |
| `GPUQueue.submit` | done | a command buffer submitted before, invalid, or of another device → validation error, nothing runs |
| `GPUQueue.writeBuffer` | done | `bufferOffset` EnforceRange; `dataOffset`/`size` in elements of a typed array, bytes of an `ArrayBuffer`/`DataView`; the view's own range (`byteOffset`, issue #328); a look-alike object → `TypeError` (ArrayBufferBounds.spec, never reaches native code); range outside the data or size not a multiple of 4 → `OperationError`; offset/range in the buffer, `COPY_DST` → validation error |
| `GPUQueue.writeTexture` | done | the view's bytes (issue #328), `dataLayout.offset` counted in bytes from the view's start; no 256 alignment needed; too little data → validation error |
| `GPUQueue.onSubmittedWorkDone` | done | (phase 1) |
| `GPUQueue.copyExternalImageToTexture` | done | sources: `ImageBitmap`, `ImageData` (`{ data, width, height }`); `origin` (GPUOrigin2D), `flipY`; destination `premultipliedAlpha` (default false), `colorSpace` (`"srgb"`, `"display-p3"`: converted, sRGB primaries to P3), `mipLevel`, `origin`, `aspect`. Alpha conversion with the reference's integer rounding (`ImageBitmap.h` `convertAlpha`), bit-exact (ImageBitmapAlpha.spec exact-bytes cases). Every destination format the spec lists (`r8unorm`, `r16float`, `r32float`, `rg8unorm`, `rg16float`, `rg32float`, `rgba8unorm(-srgb)`, `bgra8unorm(-srgb)`, `rgb10a2unorm`, `rgba16float`, `rgba32float`), encoded on the CPU. Region outside the image or `depthOrArrayLayers > 1` → `OperationError`; closed bitmap → `InvalidStateError`; destination without `RENDER_ATTACHMENT | COPY_DST` (the spec's rule; the reference does not check it), multisampled, or of another format → validation error |
| `createImageBitmap` | done | `(image, options?)` and `(image, sx, sy, sw, sh, options?)`; sources: `ArrayBuffer` / view of PNG, JPEG or WebP bytes (the `image` crate, decoded off the JS thread), `ImageData`, `ImageBitmap`, a Blob-like object with `arrayBuffer()`. Options: `premultiplyAlpha` (`"none"` straight, else premultiplied, as the reference), and, unlike the reference's native decoder (which ignores them), the crop rectangle (outside the image is transparent, negative sizes extend back, 0 → `RangeError`), `resizeWidth`/`resizeHeight` (bilinear as the reference's node client, `resizeQuality: "pixelated"` nearest, one given → the other by aspect, 0 → `InvalidStateError`), `imageOrientation: "flipY"`, in the spec's order. A look-alike buffer → `TypeError`; bytes that do not decode → `InvalidStateError` |
| `ImageBitmap` | done | `width`, `height`, `close()` (dimensions 0, idempotent, pixels released); no global constructor, as in the reference |
| `device.importExternalTexture`, `GPUExternalTexture` | partial | source: a `GPUTexture` (`rgba8unorm`/`bgra8unorm`, `TEXTURE_BINDING`; the reference's shared-texture-memory form) → wgpu 29's `ExternalTexture` with one RGBA plane; the reference's `cropOrigin`/`cropSize` (outside → throws, as the reference), `mirrored`, `rotation` 0/90/180/270 (else `TypeError`) baked into the sample and load transforms, `colorSpace: "display-p3"` into the gamut conversion; `destroy()` idempotent. Needs wgpu's `EXTERNAL_TEXTURE` on the device (master requests it when the adapter has it); without it → `ERR_WEBGPU_UNSUPPORTED`. Bound in `createBindGroup` and rendered (phase 3: `Bound::External`; `tests/commands/external.rs`) |

## Not done, and why

| Export | Status | Reason |
|---|---|---|
| `importExternalTexture` / `copyExternalImageToTexture` with `HTMLVideoElement`, `VideoFrame`, `HTMLCanvasElement`, `OffscreenCanvas`, `HTMLImageElement` | `ERR_WEBGPU_UNSUPPORTED` | NSH has no DOM |
| `importExternalTexture` / `copyExternalImageToTexture` with a `NativeBuffer` (CVPixelBuffer, AHardwareBuffer) | `ERR_WEBGPU_UNSUPPORTED` | platform frame import is area (e)'s; wgpu needs HAL interop for zero-copy |
| `GPUQueue.drawElementImageToTexture` | not in this area | needs an NSH view-to-pixels hook (area (e) / platform) |
| `createImageBitmap` `colorSpaceConversion` | accepted, ignored | no ICC handling: the bytes as encoded (the reference's native path is inconsistent here too) |
| `createImageBitmap` `imageOrientation: "from-image"` | as `"none"` | EXIF orientation is not read |
| `GPURenderBundleEncoder` debug groups / markers | accepted, not recorded | wgpu's `RenderBundleEncoder` has none |
| A `null` hole in a pipeline's `vertex.buffers` before a used slot (NullSlots.spec "keeps the index of a vertex buffer after a null one") | wrong slot | wgpu-core 29 drops attribute-less vertex layouts from the HAL pipeline without renumbering later slots (`device/resource.rs`: `if vb_state.attributes.is_empty() { continue; }`), so slot 1's attributes read slot 0's buffer. Seen on Metal with a probe draw while writing the tests; the shipped test covers the null binding itself. Phase 3: standalone repro and issue draft in `docs/upstream/wgpu-null-vertex-slot/`; fixed upstream in wgpu 30 (#9351), so the draft asks for a 29.x backport at most |
| Pass validation timing | at `finish()` | the spec reports pass errors at `finish()` too; nsh-webgpu's own checks of an ended pass/encoder report at the call, as the spec says |

## Tests (ported from `packages/webgpu/src/__tests__`)

Snapshots in `examples/headless/snapshots/commands/` (copied from the reference), compared with the reference's
`checkImage` rule (pixelmatch threshold 0.1, anti-aliasing excluded, at most 200 pixels; `tests/commands/pixelmatch.rs`).

| Reference spec | Ported to | Notes |
|---|---|---|
| `Alpha.spec.ts` | `tests/commands/draws.rs` | `semi-opaque-cyan.png` |
| `ArrayBuffer.spec.ts` (2), (3) | `tests/commands/queue.rs` | (1) is plain JS |
| `ArrayBufferBounds.spec.ts` | `examples/headless/commands.test.tsx` | all six |
| `ComputeShader.spec.ts` | `tests/commands/compute.rs` (public API) and `src/api/compute_pass_encoder.rs` (direct and indirect dispatch, dynamic offsets) | deterministic matrices instead of `Math.random` |
| `demos/Triangle.spec.ts` | `tests/commands/draws.rs` | Simple, MSAA (`triangle.png`, `triangle-msaa.png`); Async and constants are pipeline (area (a)) cases |
| `demos/DrawInstanced.spec.ts` | `tests/commands/draws.rs` | all five, plus `drawIndexedIndirect` |
| `Texture.spec.ts` | `tests/commands/draws.rs`, `queue.rs`, `commands.test.tsx` | "Create texture and reads it" (`texture.png`), writeTexture #328 |
| `ImageData.spec.ts` | `tests/commands/image_bitmap.rs`, `commands.test.tsx` | `ref.png`; sizes, `close()`, sub-view |
| `ImageBitmapAlpha.spec.ts` | `tests/commands/image_bitmap.rs`, `commands.test.tsx` | all 19 "blob" cases (bytes: NSH has no Blob), 4 buffer cases, 2 exact-bytes cases |
| `ImageBitmapBounds.spec.ts` | `commands.test.tsx` | look-alike sources reject |
| `ImageBitmapOptions.spec.ts` | `tests/commands/image_bitmap.rs` | against the reference clients' snapshots (implemented, not ignored); `colorSpaceConversion: "none"`; the P3 `"default"` case is browser-only |
| `ImageBitmapOrigin.spec.ts` | `tests/commands/image_bitmap.rs`, `commands.test.tsx` | 5 origin cases, out-of-bounds rejects |
| `ExternalTexture.spec.ts` | `tests/commands/external.rs` | Simple (1) `f.png`, Simple (2) and flipY false `f2.png`; the NativeBuffer case is area (e)'s |
| `NullSlots.spec.ts` (vertex buffer) | `tests/commands/draws.rs` | the null binding; the pipeline-hole case is the wgpu bug above |
| `EnforceRange.spec.ts` ("GPUSize64 as a direct argument") | `tests/commands/encoder.rs`, `commands.test.tsx` | |
| `demos/RenderBundles.spec.ts` (asteroid) | not ported | needs meshes, textures and `wgpu-matrix`; bundles are covered by `tests/commands/bundles.rs` (triangle and indexed grid through bundles against the same snapshots) |
| `demos/OcclusionQuery.spec.ts`, `Blur`, `ABuffer`, `Cube`, `FractalCube` | not ported | whole demo scenes (uniforms, matrices); their commands are covered above |
