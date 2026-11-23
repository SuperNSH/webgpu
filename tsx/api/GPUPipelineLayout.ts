// GPUPipelineLayout: createPipelineLayout. A null (or undefined, or a hole)
// in bindGroupLayouts is an empty group that keeps the later indices.

import { call, WgpuPipelineLayout } from "../internal/native";
import { HANDLE, INTERNAL, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPUPipelineLayout } from "../types/spec";
import type { GPUDevice } from "./GPUDevice";

/** The bind group layouts of a pipeline. */
export class GPUPipelineLayout extends ObjectBase implements SpecGPUPipelineLayout {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUPipelineLayout" {
    return "GPUPipelineLayout";
  }

  get [Symbol.toStringTag](): string {
    return "GPUPipelineLayout";
  }
}

/** `device.createPipelineLayout(descriptor)`. */
export function createPipelineLayout(device: GPUDevice, descriptor: GPUPipelineLayoutDescriptor): GPUPipelineLayout {
  if (descriptor === undefined || descriptor === null) {
    throw new TypeError("createPipelineLayout: a descriptor is required");
  }
  // Array.from turns holes into undefined; both cross as null.
  const bindGroupLayouts = Array.from(descriptor.bindGroupLayouts ?? [], (layout) => layout ?? null);
  const id = call(() =>
    WgpuPipelineLayout.create(device[HANDLE], wire({ ...descriptor, bindGroupLayouts })),
  );
  return new GPUPipelineLayout(INTERNAL, id, descriptor.label);
}
