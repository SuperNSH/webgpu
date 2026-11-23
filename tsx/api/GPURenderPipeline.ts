// GPURenderPipeline: createRenderPipeline(Async), getBindGroupLayout.

import { call, callAsync, WgpuRenderPipeline } from "../internal/native";
import { HANDLE, INTERNAL, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPURenderPipeline } from "../types/spec";
import { GPUBindGroupLayout } from "./GPUBindGroupLayout";
import { enforceRange, U32_MAX } from "./GPUBuffer";
import type { GPUDevice } from "./GPUDevice";
import { GPUPipelineError } from "./errors";

/** A render pipeline. */
export class GPURenderPipeline extends ObjectBase implements SpecGPURenderPipeline {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPURenderPipeline" {
    return "GPURenderPipeline";
  }

  /** The layout of bind group `index` (of an `"auto"` layout, or the one given). */
  getBindGroupLayout(index: number): GPUBindGroupLayout {
    index = enforceRange(index, "getBindGroupLayout index", U32_MAX);
    const id = call(() => WgpuRenderPipeline.getBindGroupLayout(this[HANDLE], index));
    return new GPUBindGroupLayout(INTERNAL, id, undefined);
  }

  get [Symbol.toStringTag](): string {
    return "GPURenderPipeline";
  }
}

/** `device.createRenderPipeline(descriptor)`. */
export function createRenderPipeline(
  device: GPUDevice,
  descriptor: GPURenderPipelineDescriptor,
): GPURenderPipeline {
  const id = call(() => WgpuRenderPipeline.create(device[HANDLE], wire(descriptor)));
  return new GPURenderPipeline(INTERNAL, id, descriptor.label);
}

/** What `create*PipelineAsync` resolves to natively: the pipeline's id, or why it failed. */
export type AsyncPipeline = { id: number } | { error: { reason: GPUPipelineErrorReason; message: string } };

/** The pipeline id of a native `createAsync`'s result; a GPUPipelineError for an invalid pipeline. */
export function pipelineId(result: AsyncPipeline): number {
  if ("error" in result) {
    throw new GPUPipelineError(result.error.message, { reason: result.error.reason });
  }
  return result.id;
}

/**
 * `device.createRenderPipelineAsync(descriptor)`: compiled off the JS
 * thread (frames and timers keep running meanwhile), in error scopes of its
 * own; rejects with a GPUPipelineError when the pipeline is invalid, a
 * TypeError when the descriptor does not convert.
 */
export async function createRenderPipelineAsync(
  device: GPUDevice,
  descriptor: GPURenderPipelineDescriptor,
): Promise<GPURenderPipeline> {
  const result = (await callAsync(() =>
    WgpuRenderPipeline.createAsync(device[HANDLE], wire(descriptor)),
  )) as AsyncPipeline;
  return new GPURenderPipeline(INTERNAL, pipelineId(result), descriptor.label);
}
