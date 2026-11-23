# Parity: area (a) resources

`GPUBuffer`, `GPUTexture`, `GPUTextureView`, `GPUSampler`, `GPUBindGroupLayout`, `GPUBindGroup`,
`GPUPipelineLayout`, `GPUShaderModule` (+ `GPUCompilationInfo` / `GPUCompilationMessage`), `GPURenderPipeline`
(`getBindGroupLayout`), `GPUComputePipeline` (+ `createComputePipelineAsync`), `GPUQuerySet`,
`GPUSupportedFeatures`, `GPUSupportedLimits`, `GPUAdapterInfo`, against react-native-webgpu 06599f9 on wgpu 29.0.4.
Verified on macOS 27.2, Apple Silicon, Metal only (evidence: `docs/evidence/resources/`).

> Area notes from phase 2. The status of every export is in [docs/parity.md](../parity.md), which is current;
> rows here that phase 3 (integration) changed are updated and marked "phase 3".

## Done

| Export | Status | Notes |
|---|---|---|
| `device.createBuffer` | done | `size`/`usage` `[EnforceRange]` in TSX (TypeError for NaN, ∞, < 0, > 2^53-1 / 2^32-1; fractions truncate, `size: 16.9` → 16); `mappedAtCreation` with `size % 4 != 0` → `RangeError`; a usage bit outside `GPUBufferUsage` or `size > maxBufferSize` → validation error |
| `GPUBuffer.mapAsync` | done | `mode`/`offset`/`size` EnforceRange; not unmapped → `OperationError` (no GPU error while pending); wrong mode / usage, `offset % 8`, `size % 4`, out of range, destroyed buffer → validation error + `OperationError`; `unmap()`/`destroy()` first → `AbortError` |
| `GPUBuffer.getMappedRange` | done | not mapped, `offset % 8`, `size % 4`, outside the mapped range, overlapping an earlier range → `OperationError`; a copy (docs/architecture.md item 6), written back on `unmap()` for `mappedAtCreation`/`MAP_WRITE`, detached on `unmap()` |
| `GPUBuffer.unmap` / `destroy` / `size` / `usage` / `mapState` | done | `destroy()` unmaps (pending map → `AbortError`); `mapState` `unmapped`/`pending`/`mapped` |
| `device.createTexture` | done | all 101 spec `GPUTextureFormat`s (each created on this Mac with the adapter's features: `cargo-test-numbers.txt`); `1d`/`2d`/`3d`; `mipLevelCount`, `sampleCount`, `viewFormats`; `GPUExtent3D` sequence or dict, EnforceRange per member; wgpu-only formats (`r64uint`, `nv12`, `p010`, ASTC HDR) → `TypeError` |
| `GPUTexture` getters, `destroy` | done | `width`, `height`, `depthOrArrayLayers`, `mipLevelCount`, `sampleCount`, `dimension`, `format`, `usage`, `textureBindingViewDimension` |
| `GPUTexture.createView` | done | every member: `format`, `dimension` (`1d`, `2d`, `2d-array`, `cube`, `cube-array`, `3d`), `usage` (bits outside `GPUTextureUsage` → validation error), `aspect` (`all`, `stencil-only`, `depth-only`; wgpu's planes → `TypeError`), `baseMipLevel`, `mipLevelCount`, `baseArrayLayer`, `arrayLayerCount`, `label` |
| `device.createSampler` | done | every member with the spec's defaults; `compare` (comparison samplers, SamplerCompare.spec #624); `maxAnisotropy` `[Clamp] unsigned short`, clamped to 16 (wgpu's maximum), > 1 needs all three filters `linear`, 0 invalid; `lodMinClamp >= 0`, `lodMaxClamp >= lodMinClamp`; an invalid sampler is a validation error and makes bind groups using it invalid. wgpu-only address modes (`clamp-to-border`, `clamp-to-zero`) → `TypeError` |
| `device.createBindGroupLayout` | done | `buffer` (`uniform`, `storage`, `read-only-storage`, `hasDynamicOffset`, `minBindingSize`), `sampler` (`filtering`, `non-filtering`, `comparison`), `texture` (5 sample types, every view dimension, `multisampled`), `storageTexture` (`write-only`, `read-only`, `read-write`, format, view dimension), `externalTexture` (see below); none or more than one member, visibility bits outside `GPUShaderStage` → validation error and an invalid layout (pipeline layouts / bind groups made from it report too) |
| `device.createBindGroup` | done | resources: `GPUSampler`, `GPUTextureView`, `GPUTexture` (its default view, the spec's newer form), `GPUBuffer` (whole), `GPUBufferBinding` (`offset`, `size`; size 0 → validation error, as the spec says, where wgpu would read "the rest"); anything else → `TypeError` |
| `device.createPipelineLayout` | done | `bindGroupLayouts` with `null` / `undefined` / holes kept as empty slots (NullSlots.spec); `immediateSize` passed to wgpu |
| `device.createShaderModule`, `getCompilationInfo` | done | WGSL; `compilationHints`/`sourceMap` accepted, ignored (they only help Dawn / tools). Messages have `message`, `type`, `lineNum`, `linePos`, `offset`, `length`, positions in UTF-16 code units as the spec counts them (naga reports bytes: converted). The reference installs none of these fields (inventory §2); this port does |
| `device.createComputePipeline(Async)`, `GPUComputePipeline.getBindGroupLayout` | done | `layout: "auto"` or a layout, `entryPoint` optional, override `constants`; invalid → validation error; `createComputePipelineAsync` rejects with `GPUPipelineError` (`reason` `validation`/`internal`) in scopes of its own, like the render variant |
| `GPURenderPipeline.getBindGroupLayout` | done | `index` EnforceRange; past the layout → validation error. Vertex formats include the newer single-component and `unorm8x4-bgra` ones (Formats.spec) |
| `device.createQuerySet`, `GPUQuerySet` | done | `occlusion`; `timestamp` where the device has `timestamp-query` (this Mac's adapter does); `count` EnforceRange, > 4096 → validation error; `type`, `count`, `label`; `destroy()` (see below) |
| `GPUSupportedFeatures` | done | set-like (`has`, `size`, `keys`/`values`/`entries`/`forEach`, iterable); spec names only, never `""`: wgpu's WebGPU mask (minus `immediates`, which the spec does not name yet), `subgroups` (wgpu `SUBGROUP`), `texture-formats-tier1` (wgpu `TEXTURE_FORMAT_16BIT_NORM`), `core-features-and-limits` (always: a wgpu device is a core device). Every advertised name is accepted by `requiredFeatures` (FeatureNames.spec) |
| `GPUSupportedLimits` | done | all 36 spec limits (the reference's 31, `maxImmediateSize`, the four `max*In{Vertex,Fragment}Stage`); `toJSON()` (GPU.spec "JSON.stringify"); `requiredLimits`: worse than default keeps the default, alignment not a power of two / unknown name / better than the adapter → `OperationError`, negative → `TypeError` |
| `GPUAdapterInfo` | done | `vendor`, `architecture`, `device`, `description`, `subgroupMinSize`, `subgroupMaxSize`, `isFallbackAdapter`; `toJSON()` |
| `__brand` | done | a getter on each class of this area, the class name, as react-native-webgpu installs it |

## Unsupported or different, with the reason

| Export | Status | Reason |
|---|---|---|
| `GPUTexture.nativePointer` | throws `ERR_WEBGPU_UNSUPPORTED` | a Dawn `WGPUTexture*` for Skia interop; wgpu has no such handle |
| `externalTexture` bind group layout entries | done (phase 3) | needs wgpu's `EXTERNAL_TEXTURE`, which `requestDevice` enables when the adapter has it; `createBindGroup` binds a `GPUExternalTexture` (`Bound::External`, `tests/commands/external.rs` `an_imported_texture_binds_and_renders`) |
| `GPUQuerySet.destroy()` | marks the set; memory freed when the last holder drops it | wgpu's `QuerySet` has no `destroy`. Area commands must check `QuerySetEntry::destroyed()` in `beginOcclusionQuery`, `timestampWrites` and `resolveQuerySet` and report the validation error |
| `maxBindGroupsPlusVertexBuffers` | `maxBindGroups + maxVertexBuffers` (12 on a default device, below the spec's 24) | wgpu enforces the two apart; the sum is what it allows. `requiredLimits` accepts it and changes nothing |
| `max{StorageBuffers,StorageTextures}In{Vertex,Fragment}Stage` | the matching `*PerShaderStage` | wgpu's per-stage limit applies to each stage |
| `maxAnisotropy` | clamped to 16 | wgpu's (and Metal's) maximum; the spec clamps to the platform's |
| `textureBindingViewDimension` | reported, ignored | compatibility mode only; a wgpu device is a core device |
| `texture-formats-tier2`, `texture-component-swizzle`, `subgroup-size-control`, `texture-compression-unaligned`, `atomic-vec2u-min-max` | never reported; requiring one → `TypeError` | wgpu 29 has no such feature |
| `texture-formats-tier1` | = wgpu `TEXTURE_FORMAT_16BIT_NORM` | the 16-bit norm formats are what wgpu gates; the tier's other renderable/storage format additions follow wgpu's own format capabilities |
| Invalid objects | a valid wgpu object plus a recorded `valid: false` (samplers, layouts, bind groups, query sets) | wgpu cannot make an "invalid" object on request; the error is reported once, in the spec's words, and the objects made from it report again |

## Tests

- `cargo test -p nsh-webgpu --test resources` (26) + `--lib` (5): Buffer.spec (mapping rules, `buffer.png` byte for
  byte), Texture.spec (`texture.png`: 0 of 1024² pixels differ), Formats.spec, SamplerCompare.spec, ComputeShader.spec,
  NullSlots.spec (bind group layout), Shaders.spec, GPU.spec (features, limits, info), FeatureNames.spec (round trip),
  every format / view / layout entry type, query sets incl. a real occlusion query. Compute and occlusion passes are
  recorded with wgpu directly on the module's objects: those pass commands are area commands'.
- `nsh test resources.test.tsx` (28): the same specs through the TSX classes, plus EnforceRange.spec, Constants.spec,
  ErrorScope.spec, Device.spec (raw buffer binding), `__brand`, `toJSON`.
- Pixel checks against the reference's snapshots run in cargo (copies in `examples/headless/snapshots/resources/`):
  the TSX runner has no PNG decoder. The check follows `checkImage` (≤ 200 differing pixels), with a per-channel
  10% threshold in place of pixelmatch's YIQ distance.
- Each test was seen failing with the code broken on purpose: `broken-on-purpose-*.txt`.

Not verified: any GPU but this Mac's (Metal), the Vulkan/DX12/GL backends, timestamp queries actually written
(that needs area commands' `timestampWrites`).
