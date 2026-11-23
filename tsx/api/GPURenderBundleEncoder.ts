// GPURenderBundleEncoder: records draws natively; finish() replays them
// into a wgpu render bundle (src/api/render_bundle_encoder.rs).

import { call, WgpuRenderBundleEncoder } from "../internal/native";
import { HANDLE, handleOf, INTERNAL, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPURenderBundleEncoder } from "../types/spec";
import type { GPUBindGroup } from "./GPUBindGroup";
import type { GPUBuffer } from "./GPUBuffer";
import { dynamicOffsets, groupOf, i32, optU32, optU64, u32, u64 } from "./GPUCommandEncoder";
import type { GPUDevice } from "./GPUDevice";
import { bytesOf } from "./GPUQueue";
import { GPURenderBundle } from "./GPURenderBundle";
import type { GPURenderPipeline } from "./GPURenderPipeline";

/** Records draws into a render bundle. */
export class GPURenderBundleEncoder extends ObjectBase implements SpecGPURenderBundleEncoder {
  declare readonly __brand: "GPURenderBundleEncoder";

  /** Ends recording: the bundle. */
  finish(descriptor?: GPURenderBundleDescriptor): GPURenderBundle {
    const id = call(() => WgpuRenderBundleEncoder.finish(this[HANDLE], wire(descriptor ?? {})));
    return new GPURenderBundle(INTERNAL, id, descriptor?.label);
  }

  /** The pipeline the next draws use. */
  setPipeline(pipeline: GPURenderPipeline): undefined {
    const p = handleOf(pipeline, "GPURenderPipeline");
    call(() => WgpuRenderBundleEncoder.setPipeline(this[HANDLE], p));
    return undefined;
  }

  /** The index buffer. */
  setIndexBuffer(buffer: GPUBuffer, indexFormat: GPUIndexFormat, offset?: number, size?: number): undefined {
    const b = handleOf(buffer, "GPUBuffer");
    const o = optU64(offset, "offset");
    const s = optU64(size, "size");
    call(() => WgpuRenderBundleEncoder.setIndexBuffer(this[HANDLE], b, String(indexFormat), o, s));
    return undefined;
  }

  /** A vertex buffer (`null` unbinds the slot). */
  setVertexBuffer(slot: number, buffer: GPUBuffer | null | undefined, offset?: number, size?: number): undefined {
    const sl = u32(slot, "slot");
    const b = buffer === null || buffer === undefined ? null : handleOf(buffer, "GPUBuffer");
    const o = optU64(offset, "offset");
    const s = optU64(size, "size");
    call(() => WgpuRenderBundleEncoder.setVertexBuffer(this[HANDLE], sl, b, o, s));
    return undefined;
  }

  /** Draws vertices. */
  draw(vertexCount: number, instanceCount?: number, firstVertex?: number, firstInstance?: number): undefined {
    const args = [
      u32(vertexCount, "vertexCount"),
      optU32(instanceCount, "instanceCount"),
      optU32(firstVertex, "firstVertex"),
      optU32(firstInstance, "firstInstance"),
    ] as const;
    call(() => WgpuRenderBundleEncoder.draw(this[HANDLE], ...args));
    return undefined;
  }

  /** Draws indexed vertices. */
  drawIndexed(
    indexCount: number,
    instanceCount?: number,
    firstIndex?: number,
    baseVertex?: number,
    firstInstance?: number,
  ): undefined {
    const args = [
      u32(indexCount, "indexCount"),
      optU32(instanceCount, "instanceCount"),
      optU32(firstIndex, "firstIndex"),
      baseVertex === undefined ? undefined : i32(baseVertex, "baseVertex"),
      optU32(firstInstance, "firstInstance"),
    ] as const;
    call(() => WgpuRenderBundleEncoder.drawIndexed(this[HANDLE], ...args));
    return undefined;
  }

  /** Draws with arguments from a buffer. */
  drawIndirect(indirectBuffer: GPUBuffer, indirectOffset: number): undefined {
    const b = handleOf(indirectBuffer, "GPUBuffer");
    const o = u64(indirectOffset, "indirectOffset");
    call(() => WgpuRenderBundleEncoder.drawIndirect(this[HANDLE], b, o));
    return undefined;
  }

  /** Draws indexed with arguments from a buffer. */
  drawIndexedIndirect(indirectBuffer: GPUBuffer, indirectOffset: number): undefined {
    const b = handleOf(indirectBuffer, "GPUBuffer");
    const o = u64(indirectOffset, "indirectOffset");
    call(() => WgpuRenderBundleEncoder.drawIndexedIndirect(this[HANDLE], b, o));
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
    call(() => WgpuRenderBundleEncoder.setBindGroup(this[HANDLE], i, group, offsets));
    return undefined;
  }

  /** Sets immediate data: `size` elements of `data` from `dataOffset`. */
  setImmediates(rangeOffset: number, data: GPUAllowSharedBufferSource, dataOffset?: number, dataSize?: number): undefined {
    const o = u32(rangeOffset, "rangeOffset");
    const bytes = bytesOf(data, optU64(dataOffset, "dataOffset"), optU64(dataSize, "size"), "setImmediates");
    call(() => WgpuRenderBundleEncoder.setImmediates(this[HANDLE], o, bytes));
    return undefined;
  }

  /**
   * Opens a debug group. wgpu's render bundles have no debug groups, so
   * the group is not recorded (docs/parity/commands.md).
   */
  pushDebugGroup(groupLabel: string): undefined {
    void String(groupLabel);
    return undefined;
  }

  /** Closes the debug group (not recorded, see pushDebugGroup). */
  popDebugGroup(): undefined {
    return undefined;
  }

  /** Marks a point for debuggers (not recorded, see pushDebugGroup). */
  insertDebugMarker(markerLabel: string): undefined {
    void String(markerLabel);
    return undefined;
  }

  get [Symbol.toStringTag](): string {
    return "GPURenderBundleEncoder";
  }
}

/** `device.createRenderBundleEncoder(descriptor)`. */
export function createRenderBundleEncoder(
  device: GPUDevice,
  descriptor: GPURenderBundleEncoderDescriptor,
): GPURenderBundleEncoder {
  const id = call(() => WgpuRenderBundleEncoder.create(device[HANDLE], wire(descriptor)));
  return new GPURenderBundleEncoder(INTERNAL, id, descriptor.label);
}
