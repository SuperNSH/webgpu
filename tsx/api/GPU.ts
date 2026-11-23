// GPU: `navigator.gpu`.

import { call, callAsync, Wgpu } from "../internal/native";
import { INTERNAL, internal } from "../internal/object";
import { wire } from "../internal/wire";
import { GPUAdapter } from "./GPUAdapter";
import { WGSLLanguageFeatures } from "./GPUSupportedFeatures";
import type { SpecGPU } from "../types/spec";

/** The entry point of WebGPU: `navigator.gpu`. */
export class GPU implements SpecGPU {
  declare readonly __brand: "GPU";
  #wgsl: WGSLLanguageFeatures | undefined;

  constructor(token: unknown) {
    internal(token);
  }

  /** An adapter matching `options`, or `null` when there is none. */
  async requestAdapter(options?: GPURequestAdapterOptions): Promise<GPUAdapter | null> {
    const id = await callAsync(() => Wgpu.requestAdapter(wire(options ?? {})));
    return id == null ? null : new GPUAdapter(INTERNAL, id);
  }

  /** The canvas format that displays best: `"bgra8unorm"`, `"rgba8unorm"` on Android. */
  getPreferredCanvasFormat(): GPUTextureFormat {
    return Wgpu.preferredCanvasFormat() as GPUTextureFormat;
  }

  /** The WGSL language extensions the shader compiler accepts. */
  get wgslLanguageFeatures(): WGSLLanguageFeatures {
    this.#wgsl ??= new WGSLLanguageFeatures(
      INTERNAL,
      call(() => Wgpu.wgslLanguageFeatures()),
    );
    return this.#wgsl;
  }

  get [Symbol.toStringTag](): string {
    return "GPU";
  }
}

/** `navigator.gpu`. */
export const gpu = new GPU(INTERNAL);
