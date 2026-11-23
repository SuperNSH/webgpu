// GPUComputePipeline: createComputePipeline(Async), getBindGroupLayout.

import { call, callAsync, WgpuComputePipeline } from "../internal/native";
import { HANDLE, INTERNAL, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPUComputePipeline } from "../types/spec";
import { GPUBindGroupLayout } from "./GPUBindGroupLayout";
import { enforceRange, U32_MAX } from "./GPUBuffer";
import type { GPUDevice } from "./GPUDevice";
import { type AsyncPipeline, pipelineId } from "./GPURenderPipeline";

/** A compute pipeline. */
export class GPUComputePipeline extends ObjectBase implements SpecGPUComputePipeline {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUComputePipeline" {
    return "GPUComputePipeline";
  }

  /** The layout of bind group `index` (of an `"auto"` layout, or the one given). */
  getBindGroupLayout(index: number): GPUBindGroupLayout {
    index = enforceRange(index, "getBindGroupLayout index", U32_MAX);
    const id = call(() => WgpuComputePipeline.getBindGroupLayout(this[HANDLE], index));
    return new GPUBindGroupLayout(INTERNAL, id, undefined);
  }

  get [Symbol.toStringTag](): string {
    return "GPUComputePipeline";
  }
}

/** `device.createComputePipeline(descriptor)`. */
export function createComputePipeline(
  device: GPUDevice,
  descriptor: GPUComputePipelineDescriptor,
): GPUComputePipeline {
  if (descriptor === undefined || descriptor === null) {
    throw new TypeError("createComputePipeline: a descriptor is required");
  }
  const id = call(() => WgpuComputePipeline.create(device[HANDLE], wire(descriptor)));
  return new GPUComputePipeline(INTERNAL, id, descriptor.label);
}

/**
 * `device.createComputePipelineAsync(descriptor)`: compiled off the JS
 * thread, in error scopes of its own; rejects with a GPUPipelineError when
 * the pipeline is invalid, a TypeError when the descriptor does not convert.
 */
export async function createComputePipelineAsync(
  device: GPUDevice,
  descriptor: GPUComputePipelineDescriptor,
): Promise<GPUComputePipeline> {
  if (descriptor === undefined || descriptor === null) {
    throw new TypeError("createComputePipelineAsync: a descriptor is required");
  }
  const result = (await callAsync(() =>
    WgpuComputePipeline.createAsync(device[HANDLE], wire(descriptor)),
  )) as AsyncPipeline;
  return new GPUComputePipeline(INTERNAL, pipelineId(result), descriptor.label);
}
