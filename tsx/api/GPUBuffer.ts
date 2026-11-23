// GPUBuffer: createBuffer, mapping, destroy.
//
// Mapping is copy-in/copy-out: getMappedRange returns a fresh ArrayBuffer
// holding a copy of the mapped bytes; unmap() writes each range back to
// the buffer when it was mapped for writing (mappedAtCreation or
// GPUMapMode.WRITE), then detaches the ArrayBuffers as the spec does.

import { call, callAsync, Wgpu, WgpuBuffer } from "../internal/native";
import { HANDLE, INTERNAL, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPUBuffer } from "../types/spec";
import type { GPUDevice } from "./GPUDevice";

export { enforceRange, optionalRange, U32_MAX, U64_MAX, withIntegers } from "../internal/range";
import { enforceRange, optionalRange, U32_MAX, U64_MAX, withIntegers } from "../internal/range";

/** A range getMappedRange gave out. */
interface MappedRange {
  offset: number;
  size: number;
  buffer: ArrayBuffer;
}

/** Detaches `buffer` (byteLength 0), where the engine can. */
function detach(buffer: ArrayBuffer): void {
  const transfer = (buffer as ArrayBuffer & { transfer?: () => ArrayBuffer }).transfer;
  if (typeof transfer === "function") {
    try {
      transfer.call(buffer);
    } catch {
      // Detached already.
    }
  }
}

/** A block of GPU memory. */
export class GPUBuffer extends ObjectBase implements SpecGPUBuffer {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUBuffer" {
    return "GPUBuffer";
  }
  readonly #device: GPUDevice;
  readonly #size: number;
  readonly #usage: number;
  #state: GPUBufferMapState;
  #writable: boolean;
  #ranges: MappedRange[] = [];

  constructor(token: unknown, id: number, device: GPUDevice, descriptor: GPUBufferDescriptor) {
    super(token, id, descriptor?.label);
    this.#device = device;
    this.#size = descriptor.size;
    this.#usage = descriptor.usage;
    this.#state = descriptor.mappedAtCreation ? "mapped" : "unmapped";
    this.#writable = !!descriptor.mappedAtCreation;
  }

  /** Its size in bytes. */
  get size(): number {
    return this.#size;
  }

  /** Its `GPUBufferUsage` flags. */
  get usage(): number {
    return this.#usage;
  }

  /** `"unmapped"`, `"pending"` (mapAsync is waiting) or `"mapped"`. */
  get mapState(): GPUBufferMapState {
    return this.#state;
  }

  /**
   * Maps `[offset, offset + size)` for `mode` (GPUMapMode.READ or WRITE)
   * once the GPU is done with the buffer. Rejects with an OperationError
   * when the buffer is not unmapped or the map is invalid, an AbortError
   * when unmap() or destroy() comes first.
   */
  async mapAsync(mode: number, offset?: number, size?: number): Promise<undefined> {
    mode = enforceRange(mode, "mapAsync mode", U32_MAX);
    offset = optionalRange(offset, "mapAsync offset");
    size = optionalRange(size, "mapAsync size");
    if (this.#state !== "unmapped") {
      throw new DOMException("mapAsync: the buffer is mapped or mapping already", "OperationError");
    }
    const ticket = call(() => WgpuBuffer.mapAsync(this[HANDLE], mode, offset, size));
    this.#state = "pending";
    try {
      await callAsync(() => Wgpu.wait(ticket));
    } catch (error) {
      if (this.#state === "pending") this.#state = "unmapped";
      this.#device.drainErrors();
      throw error;
    }
    this.#device.drainErrors();
    if (this.#state !== "pending") {
      throw new DOMException("mapAsync: the buffer was unmapped first", "AbortError");
    }
    this.#state = "mapped";
    this.#writable = (mode & 0x2) !== 0;
    return undefined;
  }

  /**
   * A copy of the mapped bytes `[offset, offset + size)` as an ArrayBuffer.
   * Writes to it reach the buffer on unmap() when it is mapped for
   * writing. Throws an OperationError when the buffer is not mapped, or the
   * range overlaps one given out before, is outside the mapping or is
   * misaligned.
   */
  getMappedRange(offset?: number, size?: number): ArrayBuffer {
    if (this.#state !== "mapped") {
      throw new DOMException("getMappedRange: the buffer is not mapped", "OperationError");
    }
    const start = optionalRange(offset, "getMappedRange offset") ?? 0;
    size = optionalRange(size, "getMappedRange size");
    const bytes = call(() => WgpuBuffer.getMappedRange(this[HANDLE], start, size));
    const end = start + bytes.byteLength;
    if (this.#ranges.some((r) => start < r.offset + r.size && r.offset < end)) {
      throw new DOMException("getMappedRange: the range overlaps one already given out", "OperationError");
    }
    const buffer =
      bytes.byteOffset === 0 && bytes.byteLength === bytes.buffer.byteLength
        ? (bytes.buffer as ArrayBuffer)
        : (bytes.slice().buffer as ArrayBuffer);
    this.#ranges.push({ offset: start, size: bytes.byteLength, buffer });
    return buffer;
  }

  /** Ends the mapping: the GPU can use the buffer again. */
  unmap(): undefined {
    const ranges = this.#ranges;
    this.#ranges = [];
    if (this.#state === "mapped" && this.#writable) {
      for (const range of ranges) {
        if (range.buffer.byteLength === range.size && range.size > 0) {
          call(() => WgpuBuffer.writeMapped(this[HANDLE], range.offset, new Uint8Array(range.buffer)));
        }
      }
    }
    for (const range of ranges) detach(range.buffer);
    call(() => WgpuBuffer.unmap(this[HANDLE]));
    this.#state = "unmapped";
    this.#writable = false;
    return undefined;
  }

  /** Frees its memory; unmaps it first. */
  destroy(): undefined {
    if (this.#state !== "unmapped") this.unmap();
    call(() => WgpuBuffer.destroy(this[HANDLE]));
    return undefined;
  }

  get [Symbol.toStringTag](): string {
    return "GPUBuffer";
  }
}

/** `device.createBuffer(descriptor)`. */
export function createBuffer(device: GPUDevice, descriptor: GPUBufferDescriptor): GPUBuffer {
  if (descriptor === undefined || descriptor === null) throw new TypeError("createBuffer: a descriptor is required");
  descriptor = withIntegers(descriptor, "GPUBufferDescriptor", [
    ["size", U64_MAX],
    ["usage", U32_MAX],
  ]);
  descriptor.size = enforceRange(descriptor.size, "GPUBufferDescriptor.size");
  descriptor.usage = enforceRange(descriptor.usage, "GPUBufferDescriptor.usage", U32_MAX);
  const id = call(() => WgpuBuffer.create(device[HANDLE], wire(descriptor)));
  return new GPUBuffer(INTERNAL, id, device, descriptor);
}
