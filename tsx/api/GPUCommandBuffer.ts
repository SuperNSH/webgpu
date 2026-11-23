// GPUCommandBuffer: what encoder.finish() makes and queue.submit() runs.

import { ObjectBase } from "../internal/object";
import type { SpecGPUCommandBuffer } from "../types/spec";

/** Recorded commands, to submit once. */
export class GPUCommandBuffer extends ObjectBase implements SpecGPUCommandBuffer {
  declare readonly __brand: "GPUCommandBuffer";

  get [Symbol.toStringTag](): string {
    return "GPUCommandBuffer";
  }
}
