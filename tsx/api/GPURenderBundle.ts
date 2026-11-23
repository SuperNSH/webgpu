// GPURenderBundle (area: commands): what a render bundle encoder's
// finish() makes.

import { ObjectBase } from "../internal/object";
import type { SpecGPURenderBundle } from "../types/spec";

/** Recorded draws to replay in render passes. */
export class GPURenderBundle extends ObjectBase implements SpecGPURenderBundle {
  declare readonly __brand: "GPURenderBundle";

  get [Symbol.toStringTag](): string {
    return "GPURenderBundle";
  }
}
