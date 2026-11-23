// GPUBindGroupLayout: createBindGroupLayout (pipelines' getBindGroupLayout
// makes them too).

import { call, WgpuBindGroupLayout } from "../internal/native";
import { HANDLE, INTERNAL, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPUBindGroupLayout } from "../types/spec";
import { enforceRange, U32_MAX, U64_MAX, withIntegers } from "./GPUBuffer";
import type { GPUDevice } from "./GPUDevice";

/** The shape of a bind group. */
export class GPUBindGroupLayout extends ObjectBase implements SpecGPUBindGroupLayout {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUBindGroupLayout" {
    return "GPUBindGroupLayout";
  }

  get [Symbol.toStringTag](): string {
    return "GPUBindGroupLayout";
  }
}

/** `device.createBindGroupLayout(descriptor)`. */
export function createBindGroupLayout(device: GPUDevice, descriptor: GPUBindGroupLayoutDescriptor): GPUBindGroupLayout {
  if (descriptor === undefined || descriptor === null) {
    throw new TypeError("createBindGroupLayout: a descriptor is required");
  }
  const entries = Array.from(descriptor.entries ?? [], (entry) => {
    const out = withIntegers(entry, "GPUBindGroupLayoutEntry", [
      ["binding", U32_MAX],
      ["visibility", U32_MAX],
    ]) as GPUBindGroupLayoutEntry;
    out.binding = enforceRange(entry.binding, "GPUBindGroupLayoutEntry.binding", U32_MAX);
    out.visibility = enforceRange(entry.visibility, "GPUBindGroupLayoutEntry.visibility", U32_MAX);
    if (entry.buffer) out.buffer = withIntegers(entry.buffer, "GPUBufferBindingLayout", [["minBindingSize", U64_MAX]]);
    return out;
  });
  const id = call(() => WgpuBindGroupLayout.create(device[HANDLE], wire({ ...descriptor, entries })));
  return new GPUBindGroupLayout(INTERNAL, id, descriptor.label);
}
