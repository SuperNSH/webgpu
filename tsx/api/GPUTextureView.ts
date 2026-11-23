// GPUTextureView: texture.createView.

import { call, WgpuTextureView } from "../internal/native";
import { HANDLE, INTERNAL, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPUTextureView } from "../types/spec";
import { U32_MAX, withIntegers } from "./GPUBuffer";
import type { GPUTexture } from "./GPUTexture";

/** A view of a texture: what passes render to and bind groups sample. */
export class GPUTextureView extends ObjectBase implements SpecGPUTextureView {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUTextureView" {
    return "GPUTextureView";
  }

  get [Symbol.toStringTag](): string {
    return "GPUTextureView";
  }
}

/** `texture.createView(descriptor)`. */
export function createView(texture: GPUTexture, descriptor?: GPUTextureViewDescriptor): GPUTextureView {
  descriptor = withIntegers(descriptor, "GPUTextureViewDescriptor", [
    ["usage", U32_MAX],
    ["baseMipLevel", U32_MAX],
    ["mipLevelCount", U32_MAX],
    ["baseArrayLayer", U32_MAX],
    ["arrayLayerCount", U32_MAX],
  ]);
  const id = call(() => WgpuTextureView.create(texture[HANDLE], wire(descriptor)));
  return new GPUTextureView(INTERNAL, id, descriptor?.label);
}
