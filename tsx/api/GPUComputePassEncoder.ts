// GPUComputePassEncoder: records dispatches natively until end(), which
// replays them into the command encoder (src/api/compute_pass_encoder.rs).
// Begun by GPUCommandEncoder.beginComputePass.

import { call, WgpuComputePassEncoder } from "../internal/native";
import { HANDLE, handleOf, ObjectBase } from "../internal/object";
import type { SpecGPUComputePassEncoder } from "../types/spec";
import type { GPUBindGroup } from "./GPUBindGroup";
import type { GPUBuffer } from "./GPUBuffer";
import { dynamicOffsets, groupOf, optU32, optU64, u32, u64 } from "./GPUCommandEncoder";
import type { GPUComputePipeline } from "./GPUComputePipeline";
import { bytesOf } from "./GPUQueue";

/** Records the dispatches of one compute pass. */
export class GPUComputePassEncoder extends ObjectBase implements SpecGPUComputePassEncoder {
  declare readonly __brand: "GPUComputePassEncoder";

  /** The pipeline the next dispatches use. */
  setPipeline(pipeline: GPUComputePipeline): undefined {
    const p = handleOf(pipeline, "GPUComputePipeline");
    call(() => WgpuComputePassEncoder.setPipeline(this[HANDLE], p));
    return undefined;
  }

  /** Dispatches `x * y * z` workgroups. */
  dispatchWorkgroups(workgroupCountX: number, workgroupCountY?: number, workgroupCountZ?: number): undefined {
    const x = u32(workgroupCountX, "workgroupCountX");
    const y = optU32(workgroupCountY, "workgroupCountY");
    const z = optU32(workgroupCountZ, "workgroupCountZ");
    call(() => WgpuComputePassEncoder.dispatchWorkgroups(this[HANDLE], x, y, z));
    return undefined;
  }

  /** Dispatches with counts from a buffer. */
  dispatchWorkgroupsIndirect(indirectBuffer: GPUBuffer, indirectOffset: number): undefined {
    const b = handleOf(indirectBuffer, "GPUBuffer");
    const o = u64(indirectOffset, "indirectOffset");
    call(() => WgpuComputePassEncoder.dispatchWorkgroupsIndirect(this[HANDLE], b, o));
    return undefined;
  }

  /** Binds a bind group (`null` unbinds), with its dynamic offsets. */
  setBindGroup(
    index: number,
    bindGroup: GPUBindGroup | null | undefined,
    dynamicOffsetsData?: Iterable<number> | Uint32Array,
    dynamicOffsetsDataStart?: number,
    dynamicOffsetsDataLength?: number,
  ): undefined {
    const i = u32(index, "index");
    const group = groupOf(bindGroup);
    const offsets = dynamicOffsets(dynamicOffsetsData, dynamicOffsetsDataStart, dynamicOffsetsDataLength, arguments.length);
    call(() => WgpuComputePassEncoder.setBindGroup(this[HANDLE], i, group, offsets));
    return undefined;
  }

  /** Sets immediate data: `size` elements of `data` from `dataOffset`. */
  setImmediates(rangeOffset: number, data: GPUAllowSharedBufferSource, dataOffset?: number, dataSize?: number): undefined {
    const o = u32(rangeOffset, "rangeOffset");
    const bytes = bytesOf(data, optU64(dataOffset, "dataOffset"), optU64(dataSize, "size"), "setImmediates");
    call(() => WgpuComputePassEncoder.setImmediates(this[HANDLE], o, bytes));
    return undefined;
  }

  /** Opens a debug group. */
  pushDebugGroup(groupLabel: string): undefined {
    call(() => WgpuComputePassEncoder.pushDebugGroup(this[HANDLE], String(groupLabel)));
    return undefined;
  }

  /** Closes the debug group. */
  popDebugGroup(): undefined {
    call(() => WgpuComputePassEncoder.popDebugGroup(this[HANDLE]));
    return undefined;
  }

  /** Marks a point for debuggers. */
  insertDebugMarker(markerLabel: string): undefined {
    call(() => WgpuComputePassEncoder.insertDebugMarker(this[HANDLE], String(markerLabel)));
    return undefined;
  }

  /** Ends the pass: its commands go into the encoder, which is unlocked. */
  end(): undefined {
    call(() => WgpuComputePassEncoder.end(this[HANDLE]));
    return undefined;
  }

  get [Symbol.toStringTag](): string {
    return "GPUComputePassEncoder";
  }
}
