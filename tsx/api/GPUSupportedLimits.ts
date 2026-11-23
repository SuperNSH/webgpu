// GPUSupportedLimits: the limits by the spec's names, from the native
// side's table (src/api/limits.rs), which has a value for every limit the
// spec defines (the combined and per-stage ones derived from wgpu's).

import { internal } from "../internal/object";

/** An adapter's or a device's limits. */
export class GPUSupportedLimits {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUSupportedLimits" {
    return "GPUSupportedLimits";
  }
  readonly #limits: Readonly<Record<string, number>>;

  constructor(token: unknown, limits: Record<string, number>) {
    internal(token);
    this.#limits = { ...limits };
  }

  #get(name: string): number {
    return this.#limits[name] ?? 0;
  }

  /** `maxTextureDimension1D`. */
  get maxTextureDimension1D(): number {
    return this.#get("maxTextureDimension1D");
  }

  /** `maxTextureDimension2D`. */
  get maxTextureDimension2D(): number {
    return this.#get("maxTextureDimension2D");
  }

  /** `maxTextureDimension3D`. */
  get maxTextureDimension3D(): number {
    return this.#get("maxTextureDimension3D");
  }

  /** `maxTextureArrayLayers`. */
  get maxTextureArrayLayers(): number {
    return this.#get("maxTextureArrayLayers");
  }

  /** `maxBindGroups`. */
  get maxBindGroups(): number {
    return this.#get("maxBindGroups");
  }

  /** `maxBindGroupsPlusVertexBuffers`. */
  get maxBindGroupsPlusVertexBuffers(): number {
    return this.#get("maxBindGroupsPlusVertexBuffers");
  }

  /** `maxBindingsPerBindGroup`. */
  get maxBindingsPerBindGroup(): number {
    return this.#get("maxBindingsPerBindGroup");
  }

  /** `maxDynamicUniformBuffersPerPipelineLayout`. */
  get maxDynamicUniformBuffersPerPipelineLayout(): number {
    return this.#get("maxDynamicUniformBuffersPerPipelineLayout");
  }

  /** `maxDynamicStorageBuffersPerPipelineLayout`. */
  get maxDynamicStorageBuffersPerPipelineLayout(): number {
    return this.#get("maxDynamicStorageBuffersPerPipelineLayout");
  }

  /** `maxSampledTexturesPerShaderStage`. */
  get maxSampledTexturesPerShaderStage(): number {
    return this.#get("maxSampledTexturesPerShaderStage");
  }

  /** `maxSamplersPerShaderStage`. */
  get maxSamplersPerShaderStage(): number {
    return this.#get("maxSamplersPerShaderStage");
  }

  /** `maxStorageBuffersPerShaderStage`. */
  get maxStorageBuffersPerShaderStage(): number {
    return this.#get("maxStorageBuffersPerShaderStage");
  }

  /** `maxStorageTexturesPerShaderStage`. */
  get maxStorageTexturesPerShaderStage(): number {
    return this.#get("maxStorageTexturesPerShaderStage");
  }

  /** `maxUniformBuffersPerShaderStage`. */
  get maxUniformBuffersPerShaderStage(): number {
    return this.#get("maxUniformBuffersPerShaderStage");
  }

  /** `maxUniformBufferBindingSize`. */
  get maxUniformBufferBindingSize(): number {
    return this.#get("maxUniformBufferBindingSize");
  }

  /** `maxStorageBufferBindingSize`. */
  get maxStorageBufferBindingSize(): number {
    return this.#get("maxStorageBufferBindingSize");
  }

  /** `minUniformBufferOffsetAlignment`. */
  get minUniformBufferOffsetAlignment(): number {
    return this.#get("minUniformBufferOffsetAlignment");
  }

  /** `minStorageBufferOffsetAlignment`. */
  get minStorageBufferOffsetAlignment(): number {
    return this.#get("minStorageBufferOffsetAlignment");
  }

  /** `maxVertexBuffers`. */
  get maxVertexBuffers(): number {
    return this.#get("maxVertexBuffers");
  }

  /** `maxBufferSize`. */
  get maxBufferSize(): number {
    return this.#get("maxBufferSize");
  }

  /** `maxVertexAttributes`. */
  get maxVertexAttributes(): number {
    return this.#get("maxVertexAttributes");
  }

  /** `maxVertexBufferArrayStride`. */
  get maxVertexBufferArrayStride(): number {
    return this.#get("maxVertexBufferArrayStride");
  }

  /** `maxInterStageShaderVariables`. */
  get maxInterStageShaderVariables(): number {
    return this.#get("maxInterStageShaderVariables");
  }

  /** `maxColorAttachments`. */
  get maxColorAttachments(): number {
    return this.#get("maxColorAttachments");
  }

  /** `maxColorAttachmentBytesPerSample`. */
  get maxColorAttachmentBytesPerSample(): number {
    return this.#get("maxColorAttachmentBytesPerSample");
  }

  /** `maxComputeWorkgroupStorageSize`. */
  get maxComputeWorkgroupStorageSize(): number {
    return this.#get("maxComputeWorkgroupStorageSize");
  }

  /** `maxComputeInvocationsPerWorkgroup`. */
  get maxComputeInvocationsPerWorkgroup(): number {
    return this.#get("maxComputeInvocationsPerWorkgroup");
  }

  /** `maxComputeWorkgroupSizeX`. */
  get maxComputeWorkgroupSizeX(): number {
    return this.#get("maxComputeWorkgroupSizeX");
  }

  /** `maxComputeWorkgroupSizeY`. */
  get maxComputeWorkgroupSizeY(): number {
    return this.#get("maxComputeWorkgroupSizeY");
  }

  /** `maxComputeWorkgroupSizeZ`. */
  get maxComputeWorkgroupSizeZ(): number {
    return this.#get("maxComputeWorkgroupSizeZ");
  }

  /** `maxComputeWorkgroupsPerDimension`. */
  get maxComputeWorkgroupsPerDimension(): number {
    return this.#get("maxComputeWorkgroupsPerDimension");
  }

  /** `maxImmediateSize`. */
  get maxImmediateSize(): number {
    return this.#get("maxImmediateSize");
  }

  /** `maxStorageBuffersInVertexStage` (wgpu's per-shader-stage limit applies to each stage). */
  get maxStorageBuffersInVertexStage(): number {
    return this.#get("maxStorageBuffersInVertexStage");
  }

  /** `maxStorageBuffersInFragmentStage` (wgpu's per-shader-stage limit applies to each stage). */
  get maxStorageBuffersInFragmentStage(): number {
    return this.#get("maxStorageBuffersInFragmentStage");
  }

  /** `maxStorageTexturesInVertexStage` (wgpu's per-shader-stage limit applies to each stage). */
  get maxStorageTexturesInVertexStage(): number {
    return this.#get("maxStorageTexturesInVertexStage");
  }

  /** `maxStorageTexturesInFragmentStage` (wgpu's per-shader-stage limit applies to each stage). */
  get maxStorageTexturesInFragmentStage(): number {
    return this.#get("maxStorageTexturesInFragmentStage");
  }

  /** Every limit by name, so `JSON.stringify(limits)` lists them (its getters are on the prototype). */
  toJSON(): Record<string, number> {
    const out: Record<string, number> = {};
    for (const name of Object.getOwnPropertyNames(GPUSupportedLimits.prototype)) {
      if (name.startsWith("max") || name.startsWith("min")) {
        out[name] = (this as unknown as Record<string, number>)[name];
      }
    }
    return out;
  }

  get [Symbol.toStringTag](): string {
    return "GPUSupportedLimits";
  }
}
