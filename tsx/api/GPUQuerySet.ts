// GPUQuerySet: createQuerySet, destroy.

import { call, WgpuQuerySet } from "../internal/native";
import { HANDLE, INTERNAL, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPUQuerySet } from "../types/spec";
import { enforceRange, U32_MAX } from "./GPUBuffer";
import type { GPUDevice } from "./GPUDevice";

/** A set of occlusion or timestamp queries. */
export class GPUQuerySet extends ObjectBase implements SpecGPUQuerySet {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUQuerySet" {
    return "GPUQuerySet";
  }
  readonly #type: GPUQueryType;
  readonly #count: number;

  constructor(token: unknown, id: number, descriptor: GPUQuerySetDescriptor) {
    super(token, id, descriptor?.label);
    this.#type = descriptor.type;
    this.#count = descriptor.count;
  }

  /** `"occlusion"` or `"timestamp"`. */
  get type(): GPUQueryType {
    return this.#type;
  }

  /** How many queries it holds. */
  get count(): number {
    return this.#count;
  }

  /** Frees it: using it after is a validation error. */
  destroy(): undefined {
    call(() => WgpuQuerySet.destroy(this[HANDLE]));
    return undefined;
  }

  get [Symbol.toStringTag](): string {
    return "GPUQuerySet";
  }
}

/** `device.createQuerySet(descriptor)`. */
export function createQuerySet(device: GPUDevice, descriptor: GPUQuerySetDescriptor): GPUQuerySet {
  if (descriptor === undefined || descriptor === null) throw new TypeError("createQuerySet: a descriptor is required");
  const desc: GPUQuerySetDescriptor = {
    ...descriptor,
    count: enforceRange(descriptor.count, "GPUQuerySetDescriptor.count", U32_MAX),
  };
  const id = call(() => WgpuQuerySet.create(device[HANDLE], wire(desc)));
  return new GPUQuerySet(INTERNAL, id, desc);
}
