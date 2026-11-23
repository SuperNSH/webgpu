// GPUExternalTexture: device.importExternalTexture (src/api/external_texture.rs).
// A GPUTexture source (react-native-webgpu's extension; rgba8unorm or
// bgra8unorm with TEXTURE_BINDING) becomes a wgpu external texture, with
// cropOrigin/cropSize, mirrored and rotation. The spec's video sources and
// react-native-webgpu's NativeBuffer source are ERR_WEBGPU_UNSUPPORTED.

import { call, unsupported, WgpuExternalTexture } from "../internal/native";
import { HANDLE, INTERNAL, isHandled, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { GPUNativeExternalTextureDescriptor } from "../types";
import type { SpecGPUExternalTexture } from "../types/spec";
import type { GPUDevice } from "./GPUDevice";

/** A texture sampled through `texture_external`. */
export class GPUExternalTexture extends ObjectBase implements SpecGPUExternalTexture {
  declare readonly __brand: "GPUExternalTexture";

  /** Releases the source (react-native-webgpu's extension); idempotent. */
  destroy(): void {
    call(() => WgpuExternalTexture.destroy(this[HANDLE]));
  }

  get [Symbol.toStringTag](): string {
    return "GPUExternalTexture";
  }
}

/** `device.importExternalTexture(descriptor)`. */
export function importExternalTexture(
  device: GPUDevice,
  descriptor: GPUExternalTextureDescriptor | GPUNativeExternalTextureDescriptor,
): GPUExternalTexture {
  const source = (descriptor as { source: unknown }).source;
  if (!isHandled(source) || Object.prototype.toString.call(source) !== "[object GPUTexture]") {
    return unsupported("importExternalTexture from this source (only a GPUTexture)");
  }
  const id = call(() => WgpuExternalTexture.import(device[HANDLE], wire(descriptor)));
  return new GPUExternalTexture(INTERNAL, id, descriptor.label);
}
