// GPUAdapter: info, features, limits and requestDevice.

import { call, callAsync, WgpuAdapter } from "../internal/native";
import { HANDLE, INTERNAL, internal, track } from "../internal/object";
import { wire } from "../internal/wire";
import { type AdapterInfoData, GPUAdapterInfo } from "./GPUAdapterInfo";
import { GPUDevice } from "./GPUDevice";
import { GPUSupportedFeatures } from "./GPUSupportedFeatures";
import { GPUSupportedLimits } from "./GPUSupportedLimits";
import type { SpecGPUAdapter } from "../types/spec";

/** A GPU and its driver: where devices come from. */
export class GPUAdapter implements SpecGPUAdapter {
  declare readonly __brand: "GPUAdapter";
  readonly [HANDLE]: number;
  readonly #features: GPUSupportedFeatures;
  readonly #limits: GPUSupportedLimits;
  readonly #info: GPUAdapterInfo;

  constructor(token: unknown, id: number) {
    internal(token);
    this[HANDLE] = id;
    track(this, id);
    this.#features = new GPUSupportedFeatures(INTERNAL, call(() => WgpuAdapter.features(id)));
    this.#limits = new GPUSupportedLimits(
      INTERNAL,
      call(() => WgpuAdapter.limits(id)) as Record<string, number>,
    );
    this.#info = new GPUAdapterInfo(INTERNAL, call(() => WgpuAdapter.info(id)) as AdapterInfoData);
  }

  /** The WebGPU features it can give a device. */
  get features(): GPUSupportedFeatures {
    return this.#features;
  }

  /** The best limits it can give a device. */
  get limits(): GPUSupportedLimits {
    return this.#limits;
  }

  /** Who made it and what it is. */
  get info(): GPUAdapterInfo {
    return this.#info;
  }

  /**
   * A device with `descriptor`'s features and limits. Rejects with a
   * TypeError for a feature the adapter lacks, an OperationError for a
   * limit it cannot give or a second request.
   */
  async requestDevice(descriptor?: GPUDeviceDescriptor): Promise<GPUDevice> {
    // WebIDL: each sequence<GPUFeatureName> member is converted to a string.
    const desc =
      descriptor?.requiredFeatures === undefined
        ? (descriptor ?? {})
        : { ...descriptor, requiredFeatures: Array.from(descriptor.requiredFeatures, (f) => String(f)) };
    const id = await callAsync(() => WgpuAdapter.requestDevice(this[HANDLE], wire(desc)));
    return new GPUDevice(INTERNAL, id, this.#info, descriptor ?? {});
  }

  /** Its attributes, as a browser's JSON.stringify shows them. */
  toJSON(): { features: GPUSupportedFeatures; limits: GPUSupportedLimits; info: GPUAdapterInfo } {
    return { features: this.#features, limits: this.#limits, info: this.#info };
  }

  get [Symbol.toStringTag](): string {
    return "GPUAdapter";
  }
}
