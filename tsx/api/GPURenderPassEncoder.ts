// GPURenderPassEncoder: records draws natively until end(), which replays
// them into the command encoder (src/api/render_pass_encoder.rs).

import { call, WgpuRenderPassEncoder } from "../internal/native";
import { HANDLE, handleOf, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPURenderPassEncoder } from "../types/spec";
import type { GPUBindGroup } from "./GPUBindGroup";
import type { GPUBuffer } from "./GPUBuffer";
import { dynamicOffsets, f32, groupOf, i32, optU32, optU64, u32, u64 } from "./GPUCommandEncoder";
import { bytesOf } from "./GPUQueue";
import type { GPURenderBundle } from "./GPURenderBundle";
import type { GPURenderPipeline } from "./GPURenderPipeline";

/** Records the draws of one render pass. */
export class GPURenderPassEncoder extends ObjectBase implements SpecGPURenderPassEncoder {
  declare readonly __brand: "GPURenderPassEncoder";

  /** The pipeline the next draws use. */
  setPipeline(pipeline: GPURenderPipeline): undefined {
    const p = handleOf(pipeline, "GPURenderPipeline");
    call(() => WgpuRenderPassEncoder.setPipeline(this[HANDLE], p));
    return undefined;
  }

  /** Draws `vertexCount` vertices, `instanceCount` times. */
  draw(vertexCount: number, instanceCount?: number, firstVertex?: number, firstInstance?: number): undefined {
    const args = [
      u32(vertexCount, "vertexCount"),
      optU32(instanceCount, "instanceCount"),
      optU32(firstVertex, "firstVertex"),
      optU32(firstInstance, "firstInstance"),
    ] as const;
    call(() => WgpuRenderPassEncoder.draw(this[HANDLE], ...args));
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
    call(() => WgpuRenderPassEncoder.drawIndexed(this[HANDLE], ...args));
    return undefined;
  }

  /** Draws with arguments from a buffer. */
  drawIndirect(indirectBuffer: GPUBuffer, indirectOffset: number): undefined {
    const b = handleOf(indirectBuffer, "GPUBuffer");
    const o = u64(indirectOffset, "indirectOffset");
    call(() => WgpuRenderPassEncoder.drawIndirect(this[HANDLE], b, o));
    return undefined;
  }

  /** Draws indexed with arguments from a buffer. */
  drawIndexedIndirect(indirectBuffer: GPUBuffer, indirectOffset: number): undefined {
    const b = handleOf(indirectBuffer, "GPUBuffer");
    const o = u64(indirectOffset, "indirectOffset");
    call(() => WgpuRenderPassEncoder.drawIndexedIndirect(this[HANDLE], b, o));
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
    call(() => WgpuRenderPassEncoder.setBindGroup(this[HANDLE], i, group, offsets));
    return undefined;
  }

  /** The index buffer. */
  setIndexBuffer(buffer: GPUBuffer, indexFormat: GPUIndexFormat, offset?: number, size?: number): undefined {
    const b = handleOf(buffer, "GPUBuffer");
    const o = optU64(offset, "offset");
    const s = optU64(size, "size");
    call(() => WgpuRenderPassEncoder.setIndexBuffer(this[HANDLE], b, String(indexFormat), o, s));
    return undefined;
  }

  /** A vertex buffer (`null` unbinds the slot). */
  setVertexBuffer(slot: number, buffer: GPUBuffer | null | undefined, offset?: number, size?: number): undefined {
    const sl = u32(slot, "slot");
    const b = buffer === null || buffer === undefined ? null : handleOf(buffer, "GPUBuffer");
    const o = optU64(offset, "offset");
    const s = optU64(size, "size");
    call(() => WgpuRenderPassEncoder.setVertexBuffer(this[HANDLE], sl, b, o, s));
    return undefined;
  }

  /** Sets immediate data: `size` elements of `data` from `dataOffset`. */
  setImmediates(rangeOffset: number, data: GPUAllowSharedBufferSource, dataOffset?: number, dataSize?: number): undefined {
    const o = u32(rangeOffset, "rangeOffset");
    const bytes = bytesOf(data, optU64(dataOffset, "dataOffset"), optU64(dataSize, "size"), "setImmediates");
    call(() => WgpuRenderPassEncoder.setImmediates(this[HANDLE], o, bytes));
    return undefined;
  }

  /** The viewport. */
  setViewport(x: number, y: number, width: number, height: number, minDepth: number, maxDepth: number): undefined {
    const args = [
      f32(x, "x"),
      f32(y, "y"),
      f32(width, "width"),
      f32(height, "height"),
      f32(minDepth, "minDepth"),
      f32(maxDepth, "maxDepth"),
    ] as const;
    call(() => WgpuRenderPassEncoder.setViewport(this[HANDLE], ...args));
    return undefined;
  }

  /** The scissor rectangle. */
  setScissorRect(x: number, y: number, width: number, height: number): undefined {
    const args = [u32(x, "x"), u32(y, "y"), u32(width, "width"), u32(height, "height")] as const;
    call(() => WgpuRenderPassEncoder.setScissorRect(this[HANDLE], ...args));
    return undefined;
  }

  /** The blend constant. */
  setBlendConstant(color: GPUColor): undefined {
    const c = wire(color);
    call(() => WgpuRenderPassEncoder.setBlendConstant(this[HANDLE], c));
    return undefined;
  }

  /** The stencil reference. */
  setStencilReference(reference: number): undefined {
    const r = u32(reference, "reference");
    call(() => WgpuRenderPassEncoder.setStencilReference(this[HANDLE], r));
    return undefined;
  }

  /** Starts occlusion query `queryIndex` of the pass's occlusion query set. */
  beginOcclusionQuery(queryIndex: number): undefined {
    const q = u32(queryIndex, "queryIndex");
    call(() => WgpuRenderPassEncoder.beginOcclusionQuery(this[HANDLE], q));
    return undefined;
  }

  /** Ends the occlusion query. */
  endOcclusionQuery(): undefined {
    call(() => WgpuRenderPassEncoder.endOcclusionQuery(this[HANDLE]));
    return undefined;
  }

  /** Runs render bundles. */
  executeBundles(bundles: Iterable<GPURenderBundle>): undefined {
    const ids = Array.from(bundles, (b) => handleOf(b, "GPURenderBundle"));
    call(() => WgpuRenderPassEncoder.executeBundles(this[HANDLE], ids));
    return undefined;
  }

  /** Opens a debug group. */
  pushDebugGroup(groupLabel: string): undefined {
    call(() => WgpuRenderPassEncoder.pushDebugGroup(this[HANDLE], String(groupLabel)));
    return undefined;
  }

  /** Closes the debug group. */
  popDebugGroup(): undefined {
    call(() => WgpuRenderPassEncoder.popDebugGroup(this[HANDLE]));
    return undefined;
  }

  /** Marks a point for debuggers. */
  insertDebugMarker(markerLabel: string): undefined {
    call(() => WgpuRenderPassEncoder.insertDebugMarker(this[HANDLE], String(markerLabel)));
    return undefined;
  }

  /** Ends the pass: its commands go into the encoder, which is unlocked. */
  end(): undefined {
    call(() => WgpuRenderPassEncoder.end(this[HANDLE]));
    return undefined;
  }

  get [Symbol.toStringTag](): string {
    return "GPURenderPassEncoder";
  }
}
