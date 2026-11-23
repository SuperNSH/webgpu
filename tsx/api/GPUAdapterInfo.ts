// GPUAdapterInfo: `adapter.info` and `device.adapterInfo`.

import { internal } from "../internal/object";

/** What `WgpuAdapter.info` reports. */
export interface AdapterInfoData {
  vendor: string;
  architecture: string;
  device: string;
  description: string;
  subgroupMinSize: number;
  subgroupMaxSize: number;
  isFallbackAdapter: boolean;
}

/** About an adapter: who made it, what it is. */
export class GPUAdapterInfo {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUAdapterInfo" {
    return "GPUAdapterInfo";
  }
  readonly #info: AdapterInfoData;

  constructor(token: unknown, info: AdapterInfoData) {
    internal(token);
    this.#info = info;
  }

  /** The maker, as the spec names it (`"apple"`, `"nvidia"`), or `""`. */
  get vendor(): string {
    return this.#info.vendor;
  }

  /** The GPU family, or `""` (wgpu does not report one). */
  get architecture(): string {
    return this.#info.architecture;
  }

  /** The vendor's device id, or `""`. */
  get device(): string {
    return this.#info.device;
  }

  /** The driver's description of the adapter. */
  get description(): string {
    return this.#info.description;
  }

  /** The smallest subgroup size. */
  get subgroupMinSize(): number {
    return this.#info.subgroupMinSize;
  }

  /** The largest subgroup size. */
  get subgroupMaxSize(): number {
    return this.#info.subgroupMaxSize;
  }

  /** Whether it is a fallback (software) adapter. */
  get isFallbackAdapter(): boolean {
    return this.#info.isFallbackAdapter;
  }

  /** Its members, so `JSON.stringify(info)` lists them (its getters are on the prototype). */
  toJSON(): AdapterInfoData {
    return { ...this.#info };
  }

  get [Symbol.toStringTag](): string {
    return "GPUAdapterInfo";
  }
}
