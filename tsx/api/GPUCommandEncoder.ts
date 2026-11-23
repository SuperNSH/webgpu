// GPUCommandEncoder: passes, copies, clears, query resolves, debug groups,
// finish(). A pass records its commands natively and replays them into the
// encoder at end() (src/api/command_encoder.rs); the encoder follows the
// spec's open / locked / ended states there.
//
// Also the WebIDL integer conversions the commands area's classes share,
// on internal/range.ts: `[EnforceRange]` arguments throw a TypeError when
// out of range and truncate fractions, as the browser's bindings do.

import { call, WgpuCommandEncoder } from "../internal/native";
import { HANDLE, handleOf, INTERNAL, ObjectBase } from "../internal/object";
import { enforceRange, optionalRange, U32_MAX, U64_MAX } from "../internal/range";
import { wire } from "../internal/wire";
import type { SpecGPUCommandEncoder } from "../types/spec";
import type { GPUBuffer } from "./GPUBuffer";
import { GPUCommandBuffer } from "./GPUCommandBuffer";
import { GPUComputePassEncoder } from "./GPUComputePassEncoder";
import type { GPUDevice } from "./GPUDevice";
import type { GPUQuerySet } from "./GPUQuerySet";
import { GPURenderPassEncoder } from "./GPURenderPassEncoder";

/** `[EnforceRange] unsigned long` (GPUSize32, GPUIndex32, GPUIntegerCoordinate, ...). */
export function u32(value: unknown, name: string): number {
  return enforceRange(value, name, U32_MAX);
}

/** `[EnforceRange] unsigned long`, or undefined when absent. */
export function optU32(value: unknown, name: string): number | undefined {
  return optionalRange(value, name, U32_MAX);
}

/** `[EnforceRange] long` (GPUSignedOffset32). */
export function i32(value: unknown, name: string): number {
  const n = Number(value);
  if (!Number.isFinite(n)) throw new TypeError(`${name}: ${String(value)} is not a finite integer`);
  const t = Math.trunc(n);
  if (t < -0x80000000 || t > 0x7fffffff) throw new TypeError(`${name}: ${String(value)} is outside [-2^31, 2^31 - 1]`);
  return t + 0;
}

/** `[EnforceRange] unsigned long long` (GPUSize64): up to 2^53 - 1 in JS. */
export function u64(value: unknown, name: string): number {
  return enforceRange(value, name, U64_MAX);
}

/** `[EnforceRange] unsigned long long`, or undefined when absent. */
export function optU64(value: unknown, name: string): number | undefined {
  return optionalRange(value, name, U64_MAX);
}

/** A WebIDL `float`: a TypeError unless finite. */
export function f32(value: unknown, name: string): number {
  const n = Number(value);
  if (!Number.isFinite(n)) throw new TypeError(`${name} (${String(value)}) is not a finite number`);
  return n;
}

/**
 * `setBindGroup`'s dynamic offsets, either form: a sequence, or a
 * `Uint32Array` with `dynamicOffsetsDataStart` and `dynamicOffsetsDataLength`
 * (a RangeError when they run past its end).
 */
export function dynamicOffsets(
  offsets: Iterable<number> | Uint32Array | undefined,
  dataStart: number | undefined,
  dataLength: number | undefined,
  argumentCount: number,
): number[] {
  if (argumentCount >= 5 || dataStart !== undefined || dataLength !== undefined) {
    if (!(offsets instanceof Uint32Array)) {
      throw new TypeError("setBindGroup: the dynamic offsets data must be a Uint32Array");
    }
    const start = u64(dataStart ?? 0, "dynamicOffsetsDataStart");
    const length = u32(dataLength ?? 0, "dynamicOffsetsDataLength");
    if (start + length > offsets.length) {
      throw new RangeError(
        `setBindGroup: dynamicOffsetsDataStart ${start} + dynamicOffsetsDataLength ${length} > ${offsets.length}`,
      );
    }
    return Array.from(offsets.subarray(start, start + length));
  }
  if (offsets === undefined || offsets === null) return [];
  return Array.from(offsets as Iterable<number>, (o, i) => u32(o, `dynamicOffsets[${i}]`));
}

/** A bind group's id, or null to unbind. */
export function groupOf(bindGroup: unknown): number | null {
  return bindGroup === null || bindGroup === undefined ? null : handleOf(bindGroup, "GPUBindGroup");
}

/** Records commands into a command buffer. */
export class GPUCommandEncoder extends ObjectBase implements SpecGPUCommandEncoder {
  declare readonly __brand: "GPUCommandEncoder";

  /** Starts a render pass; the encoder is locked until it ends. */
  beginRenderPass(descriptor: GPURenderPassDescriptor): GPURenderPassEncoder {
    const id = call(() => WgpuCommandEncoder.beginRenderPass(this[HANDLE], wire(descriptor)));
    return new GPURenderPassEncoder(INTERNAL, id, descriptor.label);
  }

  /** Starts a compute pass; the encoder is locked until it ends. */
  beginComputePass(descriptor?: GPUComputePassDescriptor): GPUComputePassEncoder {
    const id = call(() => WgpuCommandEncoder.beginComputePass(this[HANDLE], wire(descriptor ?? {})));
    return new GPUComputePassEncoder(INTERNAL, id, descriptor?.label);
  }

  /** Copies between buffers: `(source, destination, size?)` or with offsets. */
  copyBufferToBuffer(source: GPUBuffer, destination: GPUBuffer, size?: number): undefined;
  copyBufferToBuffer(
    source: GPUBuffer,
    sourceOffset: number,
    destination: GPUBuffer,
    destinationOffset: number,
    size?: number,
  ): undefined;
  copyBufferToBuffer(
    source: GPUBuffer,
    a: number | GPUBuffer,
    b?: GPUBuffer | number,
    c?: number,
    d?: number,
  ): undefined {
    const [sourceOffset, destination, destinationOffset, size] =
      typeof a === "object" ? [0, a, 0, b as number | undefined] : [a, b as GPUBuffer, c ?? 0, d];
    const src = handleOf(source, "GPUBuffer");
    const dst = handleOf(destination, "GPUBuffer");
    const so = u64(sourceOffset, "sourceOffset");
    const dso = u64(destinationOffset, "destinationOffset");
    const sz = optU64(size, "size");
    call(() => WgpuCommandEncoder.copyBufferToBuffer(this[HANDLE], src, so, dst, dso, sz));
    return undefined;
  }

  /** Copies a buffer into a texture. */
  copyBufferToTexture(
    source: GPUTexelCopyBufferInfo,
    destination: GPUTexelCopyTextureInfo,
    copySize: GPUExtent3DStrict,
  ): undefined {
    call(() =>
      WgpuCommandEncoder.copyBufferToTexture(this[HANDLE], wire(source), wire(destination), wire(copySize)),
    );
    return undefined;
  }

  /** Copies a texture into a buffer. */
  copyTextureToBuffer(
    source: GPUTexelCopyTextureInfo,
    destination: GPUTexelCopyBufferInfo,
    copySize: GPUExtent3DStrict,
  ): undefined {
    call(() =>
      WgpuCommandEncoder.copyTextureToBuffer(this[HANDLE], wire(source), wire(destination), wire(copySize)),
    );
    return undefined;
  }

  /** Copies between textures. */
  copyTextureToTexture(
    source: GPUTexelCopyTextureInfo,
    destination: GPUTexelCopyTextureInfo,
    copySize: GPUExtent3DStrict,
  ): undefined {
    call(() =>
      WgpuCommandEncoder.copyTextureToTexture(this[HANDLE], wire(source), wire(destination), wire(copySize)),
    );
    return undefined;
  }

  /** Zeroes `size` bytes of `buffer` from `offset` (to its end by default). */
  clearBuffer(buffer: GPUBuffer, offset?: number, size?: number): undefined {
    const id = handleOf(buffer, "GPUBuffer");
    const o = optU64(offset, "offset");
    const s = optU64(size, "size");
    call(() => WgpuCommandEncoder.clearBuffer(this[HANDLE], id, o, s));
    return undefined;
  }

  /** Writes query results into a buffer. */
  resolveQuerySet(
    querySet: GPUQuerySet,
    firstQuery: number,
    queryCount: number,
    destination: GPUBuffer,
    destinationOffset: number,
  ): undefined {
    const set = handleOf(querySet, "GPUQuerySet");
    const dst = handleOf(destination, "GPUBuffer");
    const first = u32(firstQuery, "firstQuery");
    const count = u32(queryCount, "queryCount");
    const offset = u64(destinationOffset, "destinationOffset");
    call(() => WgpuCommandEncoder.resolveQuerySet(this[HANDLE], set, first, count, dst, offset));
    return undefined;
  }

  /** Opens a debug group. */
  pushDebugGroup(groupLabel: string): undefined {
    call(() => WgpuCommandEncoder.pushDebugGroup(this[HANDLE], String(groupLabel)));
    return undefined;
  }

  /** Closes the debug group. */
  popDebugGroup(): undefined {
    call(() => WgpuCommandEncoder.popDebugGroup(this[HANDLE]));
    return undefined;
  }

  /** Marks a point for debuggers. */
  insertDebugMarker(markerLabel: string): undefined {
    call(() => WgpuCommandEncoder.insertDebugMarker(this[HANDLE], String(markerLabel)));
    return undefined;
  }

  /** Ends recording: the command buffer to submit. */
  finish(descriptor?: GPUCommandBufferDescriptor): GPUCommandBuffer {
    const id = call(() => WgpuCommandEncoder.finish(this[HANDLE], wire(descriptor ?? {})));
    return new GPUCommandBuffer(INTERNAL, id, descriptor?.label);
  }

  get [Symbol.toStringTag](): string {
    return "GPUCommandEncoder";
  }
}

/** `device.createCommandEncoder(descriptor)`. */
export function createCommandEncoder(device: GPUDevice, descriptor?: GPUCommandEncoderDescriptor): GPUCommandEncoder {
  const id = call(() => WgpuCommandEncoder.create(device[HANDLE], wire(descriptor ?? {})));
  return new GPUCommandEncoder(INTERNAL, id, descriptor?.label);
}
