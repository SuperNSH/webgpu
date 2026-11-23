// What every WebGPU object shares: the id of its native object, the
// guard that keeps `new GPUBuffer()` illegal as in the browser, and the
// release of the native object once JS lets the object go.

import { Wgpu } from "./native";

/** The id of an object's native half (`src/registry.rs`). */
export const HANDLE: unique symbol = Symbol.for("nsh-webgpu.handle");

/** The token nsh-webgpu's own code passes to the classes' constructors. */
export const INTERNAL: unique symbol = Symbol("nsh-webgpu.internal");

/** An object with a native half. */
export interface Handled {
  readonly [HANDLE]: number;
}

/** Throws the browser's TypeError unless `token` is nsh-webgpu's. */
export function internal(token: unknown): void {
  if (token !== INTERNAL) throw new TypeError("Illegal constructor");
}

const finalizer =
  typeof FinalizationRegistry === "function"
    ? new FinalizationRegistry<number>((id) => {
        try {
          Wgpu.release(id);
        } catch {
          // Released already.
        }
      })
    : undefined;

/** Releases native object `id` when `object` is collected. */
export function track(object: object, id: number): void {
  finalizer?.register(object, id);
}

/** Whether `value` has a native half. */
export function isHandled(value: unknown): value is Handled {
  return typeof value === "object" && value !== null && HANDLE in value;
}

/** The id of `value`, which must be a `what` (a TypeError otherwise). */
export function handleOf(value: unknown, what: string): number {
  if (!isHandled(value)) throw new TypeError(`Expected a ${what}, got ${String(value)}`);
  return value[HANDLE];
}

/**
 * `GPUObjectBase`: a label, a native half, released when collected. The
 * interfaces' classes extend it (`GPUBuffer`, `GPUTexture`, ...).
 */
export class ObjectBase implements Handled {
  /** The label, `""` by default (the spec's `GPUObjectBase.label`). */
  label: string;
  /** The native object's id. */
  readonly [HANDLE]: number;

  constructor(token: unknown, id: number, label: string | undefined) {
    internal(token);
    this[HANDLE] = id;
    this.label = label ?? "";
    track(this, id);
  }
}
