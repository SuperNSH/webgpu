// GPUBindGroup: createBindGroup. A resource is any GPUBindingResource: a
// sampler, a texture view, a texture (its default view), a buffer (all of
// it), a GPUBufferBinding or an external texture.

import { call, WgpuBindGroup } from "../internal/native";
import { HANDLE, INTERNAL, isHandled, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPUBindGroup } from "../types/spec";
import { enforceRange, U32_MAX, U64_MAX, withIntegers } from "./GPUBuffer";
import type { GPUDevice } from "./GPUDevice";

/** Resources bound together for shaders. */
export class GPUBindGroup extends ObjectBase implements SpecGPUBindGroup {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUBindGroup" {
    return "GPUBindGroup";
  }

  get [Symbol.toStringTag](): string {
    return "GPUBindGroup";
  }
}

/** `device.createBindGroup(descriptor)`. */
export function createBindGroup(device: GPUDevice, descriptor: GPUBindGroupDescriptor): GPUBindGroup {
  if (descriptor === undefined || descriptor === null) throw new TypeError("createBindGroup: a descriptor is required");
  if (!isHandled(descriptor.layout)) throw new TypeError("GPUBindGroupDescriptor.layout must be a GPUBindGroupLayout");
  const entries = Array.from(descriptor.entries ?? [], (entry) => {
    const binding = enforceRange(entry.binding, "GPUBindGroupEntry.binding", U32_MAX);
    let resource: unknown = entry.resource;
    if (!isHandled(resource)) {
      if (resource === null || typeof resource !== "object" || !isHandled((resource as GPUBufferBinding).buffer)) {
        throw new TypeError(`GPUBindGroupEntry ${binding}: resource is not a GPUBindingResource`);
      }
      resource = withIntegers(resource as GPUBufferBinding, "GPUBufferBinding", [
        ["offset", U64_MAX],
        ["size", U64_MAX],
      ]);
    }
    return { binding, resource };
  });
  const id = call(() => WgpuBindGroup.create(device[HANDLE], wire({ ...descriptor, entries })));
  return new GPUBindGroup(INTERNAL, id, descriptor.label);
}
