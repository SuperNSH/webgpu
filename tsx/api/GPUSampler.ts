// GPUSampler: createSampler.

import { call, WgpuSampler } from "../internal/native";
import { HANDLE, INTERNAL, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPUSampler } from "../types/spec";
import type { GPUDevice } from "./GPUDevice";

/** How a shader reads a texture. */
export class GPUSampler extends ObjectBase implements SpecGPUSampler {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUSampler" {
    return "GPUSampler";
  }

  get [Symbol.toStringTag](): string {
    return "GPUSampler";
  }
}

/** WebIDL `float`: a finite number, a TypeError otherwise. */
function finite(value: unknown, what: string): number {
  const n = Number(value);
  if (!Number.isFinite(n)) throw new TypeError(`${what}: ${String(value)} is not a finite number`);
  return n;
}

/** WebIDL `[Clamp] unsigned short`. */
function clampU16(value: unknown, what: string): number {
  const n = Number(value);
  if (Number.isNaN(n)) throw new TypeError(`${what}: ${String(value)} is not a number`);
  // [Clamp] rounds to the nearest integer, ties to even.
  const clamped = Math.min(Math.max(n, 0), 0xffff);
  const floor = Math.floor(clamped);
  const diff = clamped - floor;
  if (diff > 0.5) return floor + 1;
  if (diff < 0.5) return floor;
  return floor % 2 === 0 ? floor : floor + 1;
}

/** `device.createSampler(descriptor)`: invalid members are a validation error, as in the spec. */
export function createSampler(device: GPUDevice, descriptor?: GPUSamplerDescriptor): GPUSampler {
  if (descriptor !== undefined && (descriptor === null || typeof descriptor !== "object")) {
    throw new TypeError(`createSampler: expected a dictionary, got ${String(descriptor)}`);
  }
  const desc: Record<string, unknown> = { ...(descriptor ?? {}) };
  if (desc.lodMinClamp !== undefined) desc.lodMinClamp = finite(desc.lodMinClamp, "lodMinClamp");
  if (desc.lodMaxClamp !== undefined) desc.lodMaxClamp = finite(desc.lodMaxClamp, "lodMaxClamp");
  if (desc.maxAnisotropy !== undefined) desc.maxAnisotropy = clampU16(desc.maxAnisotropy, "maxAnisotropy");
  const id = call(() => WgpuSampler.create(device[HANDLE], wire(desc)));
  return new GPUSampler(INTERNAL, id, descriptor?.label);
}
