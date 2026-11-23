// GPUTexture: createTexture, createView, destroy.

import { call, unsupported, WgpuTexture } from "../internal/native";
import { HANDLE, INTERNAL, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPUTexture } from "../types/spec";
import type { GPUDevice } from "./GPUDevice";
import { enforceRange, U32_MAX, withIntegers } from "./GPUBuffer";
import { createView, type GPUTextureView } from "./GPUTextureView";

/**
 * `[width, height, depthOrArrayLayers]` of a GPUExtent3D, each a
 * `[EnforceRange] GPUIntegerCoordinate` (a TypeError otherwise, as for a
 * sequence of 0 or more than 3 members).
 */
export function extent(size: GPUExtent3DStrict): [number, number, number] {
  if (size === null || typeof size !== "object") {
    throw new TypeError(`GPUExtent3D: expected a sequence or a dictionary, got ${String(size)}`);
  }
  if (Symbol.iterator in Object(size)) {
    const members = Array.from(size as Iterable<number>);
    if (members.length < 1 || members.length > 3) {
      throw new TypeError(`GPUExtent3D: a sequence has 1 to 3 members, not ${members.length}`);
    }
    const [width, height = 1, depth = 1] = members;
    return [
      enforceRange(width, "GPUExtent3D width", U32_MAX),
      enforceRange(height, "GPUExtent3D height", U32_MAX),
      enforceRange(depth, "GPUExtent3D depthOrArrayLayers", U32_MAX),
    ];
  }
  const dict = size as GPUExtent3DDictStrict;
  return [
    enforceRange(dict.width, "GPUExtent3D width", U32_MAX),
    enforceRange(dict.height ?? 1, "GPUExtent3D height", U32_MAX),
    enforceRange(dict.depthOrArrayLayers ?? 1, "GPUExtent3D depthOrArrayLayers", U32_MAX),
  ];
}

/** An image on the GPU. */
export class GPUTexture extends ObjectBase implements SpecGPUTexture {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUTexture" {
    return "GPUTexture";
  }
  readonly #device: GPUDevice;
  readonly #size: [number, number, number];
  readonly #descriptor: GPUTextureDescriptor;

  constructor(token: unknown, id: number, device: GPUDevice, descriptor: GPUTextureDescriptor) {
    super(token, id, descriptor?.label);
    this.#device = device;
    this.#size = extent(descriptor.size);
    this.#descriptor = descriptor;
  }

  /** The device it belongs to. */
  get device(): GPUDevice {
    return this.#device;
  }

  /** A view of it. */
  createView(descriptor?: GPUTextureViewDescriptor): GPUTextureView {
    return createView(this, descriptor);
  }

  /** Frees its memory. */
  destroy(): undefined {
    call(() => WgpuTexture.destroy(this[HANDLE]));
    return undefined;
  }

  /** Width in texels. */
  get width(): number {
    return this.#size[0];
  }

  /** Height in texels. */
  get height(): number {
    return this.#size[1];
  }

  /** Depth, or the number of layers. */
  get depthOrArrayLayers(): number {
    return this.#size[2];
  }

  /** Mip levels. */
  get mipLevelCount(): number {
    return this.#descriptor.mipLevelCount ?? 1;
  }

  /** Samples per texel. */
  get sampleCount(): number {
    return this.#descriptor.sampleCount ?? 1;
  }

  /** `"1d"`, `"2d"` or `"3d"`. */
  get dimension(): GPUTextureDimension {
    return this.#descriptor.dimension ?? "2d";
  }

  /** Its format. */
  get format(): GPUTextureFormat {
    return this.#descriptor.format;
  }

  /** Its `GPUTextureUsage` flags. */
  get usage(): number {
    return this.#descriptor.usage;
  }

  /** The view dimension of its texture bindings, when it was given. */
  get textureBindingViewDimension(): GPUTextureViewDimension | undefined {
    return this.#descriptor.textureBindingViewDimension;
  }

  /**
   * react-native-webgpu's raw `WGPUTexture*` (a Dawn pointer, for Skia
   * interop). wgpu has no such handle: ERR_WEBGPU_UNSUPPORTED.
   */
  get nativePointer(): bigint {
    return unsupported("GPUTexture.nativePointer (a Dawn WGPUTexture pointer; wgpu has none)");
  }

  get [Symbol.toStringTag](): string {
    return "GPUTexture";
  }
}

/** `device.createTexture(descriptor)`. */
export function createTexture(device: GPUDevice, descriptor: GPUTextureDescriptor): GPUTexture {
  if (descriptor === undefined || descriptor === null) throw new TypeError("createTexture: a descriptor is required");
  descriptor = withIntegers(descriptor, "GPUTextureDescriptor", [
    ["mipLevelCount", U32_MAX],
    ["sampleCount", U32_MAX],
  ]);
  descriptor.usage = enforceRange(descriptor.usage, "GPUTextureDescriptor.usage", U32_MAX);
  const [width, height, depthOrArrayLayers] = extent(descriptor.size);
  descriptor.size = { width, height, depthOrArrayLayers };
  if (descriptor.viewFormats !== undefined) descriptor.viewFormats = Array.from(descriptor.viewFormats);
  const id = call(() => WgpuTexture.create(device[HANDLE], wire(descriptor)));
  return new GPUTexture(INTERNAL, id, device, descriptor);
}

/** A texture made elsewhere (a canvas's current texture), with its id. */
export function wrapTexture(device: GPUDevice, id: number, descriptor: GPUTextureDescriptor): GPUTexture {
  return new GPUTexture(INTERNAL, id, device, descriptor);
}
