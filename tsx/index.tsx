/// <reference path="./types/webgpu.d.ts" />
/// <reference path="./types/dom.d.ts" />
// nsh-webgpu: WebGPU for NSH, as react-native-webgpu exports it.
// Orchestrator-owned. The spec's names are listed (they are fixed); the
// react-native-webgpu files are re-exported whole (`export *`), so an area
// adds names in its own file without touching this one.

// First, and bare: on the web it reads the browser's `navigator.gpu`
// before anything else here is evaluated (the reference's web entry loads
// its polyfill first the same way).
import "./surface/web";
import { installWebGPU } from "./install";
import type {
  GPUCopyExternalNativeFrameSourceInfo,
  GPUDawnTogglesDescriptor,
  GPUDrawElementImageDestination,
  GPUDrawElementImageSource,
  GPUNativeExternalTextureDescriptor,
  GPUSharedFence,
  GPUSharedFenceDescriptor,
  GPUSharedTextureMemory,
  GPUSharedTextureMemoryDescriptor,
} from "./types";

// The spec's classes (one file each, the names fixed by the spec: an area
// changes its file, never this list), constants and errors.
export { GPU } from "./api/GPU";
export { GPUAdapter } from "./api/GPUAdapter";
export { GPUAdapterInfo } from "./api/GPUAdapterInfo";
export { GPUBindGroup } from "./api/GPUBindGroup";
export { GPUBindGroupLayout } from "./api/GPUBindGroupLayout";
export { GPUBuffer } from "./api/GPUBuffer";
export { GPUCanvasContext } from "./api/GPUCanvasContext";
export { GPUCommandBuffer } from "./api/GPUCommandBuffer";
export { GPUCommandEncoder } from "./api/GPUCommandEncoder";
export { GPUComputePassEncoder } from "./api/GPUComputePassEncoder";
export { GPUComputePipeline } from "./api/GPUComputePipeline";
export { GPUDevice } from "./api/GPUDevice";
export { GPUExternalTexture } from "./api/GPUExternalTexture";
export { GPUPipelineLayout } from "./api/GPUPipelineLayout";
export { GPUQuerySet } from "./api/GPUQuerySet";
export { GPUQueue } from "./api/GPUQueue";
export { GPURenderBundle } from "./api/GPURenderBundle";
export { GPURenderBundleEncoder } from "./api/GPURenderBundleEncoder";
export { GPURenderPassEncoder } from "./api/GPURenderPassEncoder";
export { GPURenderPipeline } from "./api/GPURenderPipeline";
export { GPUSampler } from "./api/GPUSampler";
export { GPUCompilationInfo, GPUCompilationMessage, GPUShaderModule } from "./api/GPUShaderModule";
export { GPUSupportedFeatures, WGSLLanguageFeatures } from "./api/GPUSupportedFeatures";
export { GPUSupportedLimits } from "./api/GPUSupportedLimits";
export { GPUTexture } from "./api/GPUTexture";
export { GPUTextureView } from "./api/GPUTextureView";
export { GPUBufferUsage, GPUColorWrite, GPUMapMode, GPUShaderStage, GPUTextureUsage } from "./api/constants";
export {
  GPUDeviceLostInfo,
  GPUError,
  GPUInternalError,
  GPUOutOfMemoryError,
  GPUPipelineError,
  GPUUncapturedErrorEvent,
  GPUValidationError,
} from "./api/errors";

// react-native-webgpu's own exports.
export * from "./Canvas";
export * from "./NativeBufferView";
export * from "./Offscreen";
export * from "./hooks";
export * from "./GPUDeviceProvider";
export * from "./importDevice";
export * from "./video";
export * from "./formats";
export * from "./types";
export { WebGPUModule } from "./WebGPUModule";
export { installWebGPU } from "./install";
export { requestAnimationFrame, cancelAnimationFrame } from "./animationFrame";

// As react-native-webgpu does on import: `navigator.gpu` and the globals.
installWebGPU();

// react-native-webgpu's augmentations of the WebGPU globals (its
// src/index.tsx), with the same members and signatures. Types only: each
// note says what nsh-webgpu does at run time (docs/parity.md). The
// reference's `RNWebGPU` and `__webgpuAnimationFrames` globals are not
// declared: nsh-webgpu installs neither (its functions are the package's
// exports). `createImageBitmap` on encoded bytes is in ./types/dom.d.ts.
declare global {
  interface GPUTexture {
    /**
     * Non-spec RN extension: the raw WGPUTexture handle. nsh-webgpu: reading
     * it throws ERR_WEBGPU_UNSUPPORTED (wgpu has no Dawn handle).
     */
    readonly nativePointer: bigint;
  }

  interface GPUDevice {
    /**
     * Non-spec RN extension: a NativeBuffer, or a texture, as the source.
     * nsh-webgpu: a GPUTexture source works (with `cropOrigin`, `cropSize`,
     * `rotation`, `mirrored`); a NativeBuffer throws ERR_WEBGPU_UNSUPPORTED.
     */
    importExternalTexture(descriptor: GPUNativeExternalTextureDescriptor): GPUExternalTexture;
    /** Non-spec RN extension. nsh-webgpu: throws ERR_WEBGPU_UNSUPPORTED (wgpu has no shared texture memory). */
    importSharedTextureMemory(descriptor: GPUSharedTextureMemoryDescriptor): GPUSharedTextureMemory;
    /** Non-spec RN extension. nsh-webgpu: throws ERR_WEBGPU_UNSUPPORTED (wgpu has no shared fences). */
    importSharedFence(descriptor: GPUSharedFenceDescriptor): GPUSharedFence;
  }

  interface GPUQueue {
    /**
     * Non-spec RN extension: rasterizes a native view into a texture.
     * nsh-webgpu: rejects with ERR_WEBGPU_UNSUPPORTED (NSH has no hook that
     * draws a view subtree into pixels).
     */
    drawElementImageToTexture(
      source: GPUDrawElementImageSource,
      destination: GPUDrawElementImageDestination,
    ): Promise<void>;
    /**
     * Non-spec RN extension: a NativeBuffer as the copy source. nsh-webgpu:
     * throws ERR_WEBGPU_UNSUPPORTED (ImageBitmap and ImageData sources work).
     */
    copyExternalImageToTexture(
      source: GPUCopyExternalNativeFrameSourceInfo,
      destination: GPUCopyExternalImageDestInfo,
      copySize: GPUExtent3DStrict,
    ): undefined;
  }

  interface GPUDeviceDescriptor {
    /** Non-standard, Dawn-only device toggles. nsh-webgpu: accepted and ignored (wgpu has no toggles). */
    dawnToggles?: GPUDawnTogglesDescriptor;
    /**
     * Non-spec RN extension: Dawn's per-device locking. nsh-webgpu: accepted
     * and ignored (a wgpu device is thread-safe without it).
     */
    implicitDeviceSynchronization?: boolean;
  }

  interface GPUExternalTextureDescriptor {
    /** Non-spec extension: the clockwise rotation baked into sampling. nsh-webgpu: applied. */
    rotation?: 0 | 90 | 180 | 270;
    /** Non-spec extension: a horizontal flip baked into sampling. nsh-webgpu: applied. */
    mirrored?: boolean;
  }

  interface GPUExternalTexture {
    /** Non-spec extension: releases the source now instead of at collection. nsh-webgpu: implemented, idempotent. */
    destroy(): void;
  }
}
