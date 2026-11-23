// GPUDevice: features, limits, the queue, errors, `lost`, `destroy()`.
// Its `create*` methods are the created kinds' (`createBuffer` is
// GPUBuffer.ts's), so the areas never edit this file for them.

import { call, unsupported, WgpuDevice } from "../internal/native";
import { HANDLE, INTERNAL, internal } from "../internal/object";
import type { SpecGPUDevice } from "../types/spec";
import type {
  GPUNativeExternalTextureDescriptor,
  GPUSharedFence,
  GPUSharedFenceDescriptor,
  GPUSharedTextureMemory,
  GPUSharedTextureMemoryDescriptor,
} from "../types";
import { createBindGroup, type GPUBindGroup } from "./GPUBindGroup";
import { createBindGroupLayout, type GPUBindGroupLayout } from "./GPUBindGroupLayout";
import { createBuffer, type GPUBuffer } from "./GPUBuffer";
import { createCommandEncoder, type GPUCommandEncoder } from "./GPUCommandEncoder";
import {
  createComputePipeline,
  createComputePipelineAsync,
  type GPUComputePipeline,
} from "./GPUComputePipeline";
import { type GPUExternalTexture, importExternalTexture } from "./GPUExternalTexture";
import { createPipelineLayout, type GPUPipelineLayout } from "./GPUPipelineLayout";
import { createQuerySet, type GPUQuerySet } from "./GPUQuerySet";
import { createRenderBundleEncoder, type GPURenderBundleEncoder } from "./GPURenderBundleEncoder";
import {
  createRenderPipeline,
  createRenderPipelineAsync,
  type GPURenderPipeline,
} from "./GPURenderPipeline";
import { createSampler, type GPUSampler } from "./GPUSampler";
import { createShaderModule, type GPUShaderModule } from "./GPUShaderModule";
import { createTexture, type GPUTexture } from "./GPUTexture";
import type { GPUAdapterInfo } from "./GPUAdapterInfo";
import { GPUQueue } from "./GPUQueue";
import { GPUSupportedFeatures } from "./GPUSupportedFeatures";
import { GPUSupportedLimits } from "./GPUSupportedLimits";
import {
  type GPUDeviceLostInfo,
  type GPUError,
  GPUUncapturedErrorEvent,
  gpuError,
  lostInfo,
} from "./errors";

type UncapturedErrorHandler = SpecGPUDevice["onuncapturederror"];

/**
 * Releases a device's native half when its GPUDevice is collected,
 * destroyed or not (`WgpuDevice.release`: its listeners and its entry).
 */
const deviceFinalizer =
  typeof FinalizationRegistry === "function"
    ? new FinalizationRegistry<number>((id) => {
        try {
          WgpuDevice.release(id);
        } catch {
          // Released already.
        }
      })
    : undefined;

/** A weak reference where the runtime has them, a strong one otherwise. */
const weakly = <T extends object>(target: T): { deref(): T | undefined } =>
  typeof WeakRef === "function" ? new WeakRef(target) : { deref: () => target };

/** A logical GPU: what makes every other object, and reports their errors. */
export class GPUDevice extends EventTarget implements SpecGPUDevice {
  declare readonly __brand: "GPUDevice";
  /** The label, `""` by default. */
  label: string;
  readonly [HANDLE]: number;
  readonly #features: GPUSupportedFeatures;
  readonly #limits: GPUSupportedLimits;
  readonly #adapterInfo: GPUAdapterInfo;
  readonly #queue: GPUQueue;
  readonly #lost: Promise<GPUDeviceLostInfo>;
  #resolveLost: ((info: GPUDeviceLostInfo) => void) | undefined;
  #handler: UncapturedErrorHandler = null;
  #handlerListener: ((event: Event) => void) | undefined;
  #listeners = 0;
  #destroyed = false;

  constructor(token: unknown, id: number, adapterInfo: GPUAdapterInfo, descriptor: GPUDeviceDescriptor) {
    super();
    internal(token);
    this[HANDLE] = id;
    this.label = descriptor.label ?? "";
    this.#adapterInfo = adapterInfo;
    this.#features = new GPUSupportedFeatures(INTERNAL, call(() => WgpuDevice.features(id)));
    this.#limits = new GPUSupportedLimits(
      INTERNAL,
      call(() => WgpuDevice.limits(id)) as Record<string, number>,
    );
    this.#queue = new GPUQueue(INTERNAL, this, descriptor.defaultQueue?.label);
    this.#lost = new Promise((resolve) => {
      this.#resolveLost = resolve;
    });
    // The native listeners hold the device weakly, so a device the app
    // drops is collected (and released by the finalizer) like any object.
    const self = weakly(this);
    WgpuDevice.onUncapturedError(id, (kind, message) => {
      const device = self.deref();
      if (device) device.#uncaptured(kind, message);
    });
    WgpuDevice.onLost(id, (reason, message) => {
      const device = self.deref();
      if (device) device.#lose(reason, message);
    });
    deviceFinalizer?.register(this, id);
  }

  /** The features it was made with. */
  get features(): GPUSupportedFeatures {
    return this.#features;
  }

  /** The limits it was made with. */
  get limits(): GPUSupportedLimits {
    return this.#limits;
  }

  /** Its adapter's info. */
  get adapterInfo(): GPUAdapterInfo {
    return this.#adapterInfo;
  }

  /** Its queue. */
  get queue(): GPUQueue {
    return this.#queue;
  }

  /** Settles when the device is lost: `destroy()`, or the GPU went away. */
  get lost(): Promise<GPUDeviceLostInfo> {
    return this.#lost;
  }

  /**
   * Destroys the device and its resources; `lost` settles with
   * `"destroyed"`, then the native device's id and listeners are released.
   */
  destroy(): undefined {
    if (this.#destroyed) return undefined;
    this.#destroyed = true;
    call(() => WgpuDevice.destroy(this[HANDLE]));
    this.#lose("destroyed", "");
    return undefined;
  }

  /** A buffer. */
  createBuffer(descriptor: GPUBufferDescriptor): GPUBuffer {
    return createBuffer(this, descriptor);
  }

  /** A texture. */
  createTexture(descriptor: GPUTextureDescriptor): GPUTexture {
    return createTexture(this, descriptor);
  }

  /** A sampler. */
  createSampler(descriptor?: GPUSamplerDescriptor): GPUSampler {
    return createSampler(this, descriptor);
  }

  /** An external texture: from a GPUTexture (react-native-webgpu's extension); other sources are ERR_WEBGPU_UNSUPPORTED. */
  importExternalTexture(
    descriptor: GPUExternalTextureDescriptor | GPUNativeExternalTextureDescriptor,
  ): GPUExternalTexture {
    return importExternalTexture(this, descriptor);
  }

  /**
   * react-native-webgpu's shared texture memory (IOSurface, AHardwareBuffer):
   * ERR_WEBGPU_UNSUPPORTED, wgpu has no shared texture memory (docs/parity.md).
   */
  importSharedTextureMemory(_descriptor: GPUSharedTextureMemoryDescriptor): GPUSharedTextureMemory {
    return unsupported("GPUDevice.importSharedTextureMemory (wgpu has no shared texture memory)");
  }

  /** react-native-webgpu's shared fences: ERR_WEBGPU_UNSUPPORTED, wgpu has no shared fences. */
  importSharedFence(_descriptor: GPUSharedFenceDescriptor): GPUSharedFence {
    return unsupported("GPUDevice.importSharedFence (wgpu has no shared fences)");
  }

  /** Dawn's testing hook: ERR_WEBGPU_UNSUPPORTED, wgpu cannot lose a device on request. */
  forceLossForTesting(): undefined {
    return unsupported("GPUDevice.forceLossForTesting (wgpu cannot lose a device on request)");
  }

  /** A bind group layout. */
  createBindGroupLayout(descriptor: GPUBindGroupLayoutDescriptor): GPUBindGroupLayout {
    return createBindGroupLayout(this, descriptor);
  }

  /** A pipeline layout. */
  createPipelineLayout(descriptor: GPUPipelineLayoutDescriptor): GPUPipelineLayout {
    return createPipelineLayout(this, descriptor);
  }

  /** A bind group. */
  createBindGroup(descriptor: GPUBindGroupDescriptor): GPUBindGroup {
    return createBindGroup(this, descriptor);
  }

  /** A shader module from WGSL. */
  createShaderModule(descriptor: GPUShaderModuleDescriptor): GPUShaderModule {
    return createShaderModule(this, descriptor);
  }

  /** A compute pipeline. */
  createComputePipeline(descriptor: GPUComputePipelineDescriptor): GPUComputePipeline {
    return createComputePipeline(this, descriptor);
  }

  /** A render pipeline. */
  createRenderPipeline(descriptor: GPURenderPipelineDescriptor): GPURenderPipeline {
    return createRenderPipeline(this, descriptor);
  }

  /** A compute pipeline; rejects with a GPUPipelineError when it is invalid. */
  createComputePipelineAsync(descriptor: GPUComputePipelineDescriptor): Promise<GPUComputePipeline> {
    return createComputePipelineAsync(this, descriptor);
  }

  /** A render pipeline; rejects with a GPUPipelineError when it is invalid. */
  createRenderPipelineAsync(descriptor: GPURenderPipelineDescriptor): Promise<GPURenderPipeline> {
    return createRenderPipelineAsync(this, descriptor);
  }

  /** A command encoder. */
  createCommandEncoder(descriptor?: GPUCommandEncoderDescriptor): GPUCommandEncoder {
    return createCommandEncoder(this, descriptor);
  }

  /** A render bundle encoder. */
  createRenderBundleEncoder(descriptor: GPURenderBundleEncoderDescriptor): GPURenderBundleEncoder {
    return createRenderBundleEncoder(this, descriptor);
  }

  /** A query set. */
  createQuerySet(descriptor: GPUQuerySetDescriptor): GPUQuerySet {
    return createQuerySet(this, descriptor);
  }

  /** Opens an error scope catching errors of kind `filter`. */
  pushErrorScope(filter: GPUErrorFilter): undefined {
    call(() => WgpuDevice.pushErrorScope(this[HANDLE], filter));
    return undefined;
  }

  /**
   * Closes the innermost error scope: the first error it caught, or `null`.
   * Rejects with an OperationError when no scope is open.
   */
  popErrorScope(): Promise<GPUError | null> {
    try {
      const caught = call(() => WgpuDevice.popErrorScope(this[HANDLE])) as {
        kind: string;
        message: string;
      } | null;
      return Promise.resolve(caught ? gpuError(caught.kind, caught.message) : null);
    } catch (error) {
      return Promise.reject(error);
    }
  }

  /** The `uncapturederror` event handler. */
  get onuncapturederror(): UncapturedErrorHandler {
    return this.#handler;
  }

  set onuncapturederror(handler: UncapturedErrorHandler) {
    if (this.#handlerListener) super.removeEventListener("uncapturederror", this.#handlerListener);
    this.#handler = typeof handler === "function" ? handler : null;
    this.#handlerListener = undefined;
    if (this.#handler) {
      const handle = this.#handler;
      this.#handlerListener = (event) =>
        handle.call(this as unknown as SpecGPUDevice, event as unknown as SpecGPUUncapturedErrorEventLike);
      super.addEventListener("uncapturederror", this.#handlerListener);
    }
  }

  override addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: AddEventListenerOptions | boolean,
  ): void {
    if (type === "uncapturederror" && listener) this.#listeners++;
    super.addEventListener(type, listener, options);
  }

  override removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: { capture?: boolean } | boolean,
  ): void {
    if (type === "uncapturederror" && listener) this.#listeners = Math.max(0, this.#listeners - 1);
    super.removeEventListener(type, listener, options);
  }

  /**
   * Dispatches the uncaptured errors that arrived while the device was
   * polled off the JS thread (called after each wait: mapAsync,
   * onSubmittedWorkDone).
   */
  drainErrors(): void {
    for (const error of call(() => WgpuDevice.takeUncapturedErrors(this[HANDLE])) as {
      kind: string;
      message: string;
    }[]) {
      this.#uncaptured(error.kind, error.message);
    }
    const lost = call(() => WgpuDevice.lostInfo(this[HANDLE])) as { reason: string; message: string } | null;
    if (lost) this.#lose(lost.reason, lost.message);
  }

  #uncaptured(kind: string, message: string): void {
    const error = gpuError(kind, message);
    const event = new GPUUncapturedErrorEvent("uncapturederror", { error, cancelable: true });
    this.dispatchEvent(event);
    // As browsers do: an error nobody heard goes to the console.
    if (this.#listeners === 0 && !this.#handler) {
      console.warn(`[nsh-webgpu] uncaptured ${kind} error: ${message}`);
    }
  }

  #lose(reason: string, message: string): void {
    const resolve = this.#resolveLost;
    if (!resolve) return;
    this.#resolveLost = undefined;
    resolve(lostInfo(reason, message));
  }

  get [Symbol.toStringTag](): string {
    return "GPUDevice";
  }
}

/** What `onuncapturederror` handlers take, as the spec types it. */
type SpecGPUUncapturedErrorEventLike = Parameters<NonNullable<UncapturedErrorHandler>>[0];
