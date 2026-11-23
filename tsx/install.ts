// installWebGPU(): `navigator.gpu`, the GPU* classes and the flag constants
// as globals, as react-native-webgpu's native module installs them. The
// package's entry calls it when it is imported; call it again in a
// runtime that started without them. On the web it does nothing: the
// browser's own WebGPU stays (the reference's web entry installs no `gpu`).

import { platform } from "nsh";

import { GPUBufferUsage, GPUColorWrite, GPUMapMode, GPUShaderStage, GPUTextureUsage } from "./api/constants";
import { GPU, gpu } from "./api/GPU";
import { GPUAdapter } from "./api/GPUAdapter";
import { GPUAdapterInfo } from "./api/GPUAdapterInfo";
import { GPUBindGroup } from "./api/GPUBindGroup";
import { GPUBindGroupLayout } from "./api/GPUBindGroupLayout";
import { GPUBuffer } from "./api/GPUBuffer";
import { GPUCanvasContext } from "./api/GPUCanvasContext";
import { GPUCommandBuffer } from "./api/GPUCommandBuffer";
import { GPUCommandEncoder } from "./api/GPUCommandEncoder";
import { GPUComputePassEncoder } from "./api/GPUComputePassEncoder";
import { GPUComputePipeline } from "./api/GPUComputePipeline";
import { GPUDevice } from "./api/GPUDevice";
import { GPUExternalTexture } from "./api/GPUExternalTexture";
import { GPUPipelineLayout } from "./api/GPUPipelineLayout";
import { GPUQuerySet } from "./api/GPUQuerySet";
import { GPUQueue } from "./api/GPUQueue";
import { GPURenderBundle } from "./api/GPURenderBundle";
import { GPURenderBundleEncoder } from "./api/GPURenderBundleEncoder";
import { GPURenderPassEncoder } from "./api/GPURenderPassEncoder";
import { GPURenderPipeline } from "./api/GPURenderPipeline";
import { GPUSampler } from "./api/GPUSampler";
import { GPUCompilationInfo, GPUCompilationMessage, GPUShaderModule } from "./api/GPUShaderModule";
import { GPUSupportedFeatures, WGSLLanguageFeatures } from "./api/GPUSupportedFeatures";
import { GPUSupportedLimits } from "./api/GPUSupportedLimits";
import { GPUTexture } from "./api/GPUTexture";
import { GPUTextureView } from "./api/GPUTextureView";
import { createImageBitmap } from "./imageBitmap";
import {
  GPUDeviceLostInfo,
  GPUError,
  GPUInternalError,
  GPUOutOfMemoryError,
  GPUPipelineError,
  GPUUncapturedErrorEvent,
  GPUValidationError,
} from "./api/errors";

/** The globals WebGPU defines, by name. */
const GLOBALS: Record<string, unknown> = {
  GPU,
  GPUAdapter,
  GPUAdapterInfo,
  GPUBindGroup,
  GPUBindGroupLayout,
  GPUBuffer,
  GPUCanvasContext,
  GPUCommandBuffer,
  GPUCommandEncoder,
  GPUCompilationInfo,
  GPUCompilationMessage,
  GPUComputePassEncoder,
  GPUComputePipeline,
  GPUDevice,
  GPUDeviceLostInfo,
  GPUError,
  GPUExternalTexture,
  GPUInternalError,
  GPUOutOfMemoryError,
  GPUPipelineError,
  GPUPipelineLayout,
  GPUQuerySet,
  GPUQueue,
  GPURenderBundle,
  GPURenderBundleEncoder,
  GPURenderPassEncoder,
  GPURenderPipeline,
  GPUSampler,
  GPUShaderModule,
  GPUSupportedFeatures,
  GPUSupportedLimits,
  GPUTexture,
  GPUTextureView,
  GPUUncapturedErrorEvent,
  GPUValidationError,
  WGSLLanguageFeatures,
  GPUBufferUsage,
  GPUTextureUsage,
  GPUShaderStage,
  GPUMapMode,
  GPUColorWrite,
};

/**
 * Puts `navigator.gpu`, the GPU* classes and the flag constants on
 * `globalThis`. A no-op on the web, where the browser's are kept.
 */
export const installWebGPU = (): void => {
  if (platform.os === "web") return;
  const global = globalThis as unknown as Record<string, unknown> & { navigator?: Record<string, unknown> };
  for (const [name, value] of Object.entries(GLOBALS)) {
    Object.defineProperty(global, name, { value, writable: true, configurable: true, enumerable: false });
  }
  // react-native-webgpu's: the runtime's own when it has one.
  if (typeof global.createImageBitmap !== "function") {
    Object.defineProperty(global, "createImageBitmap", {
      value: createImageBitmap,
      writable: true,
      configurable: true,
      enumerable: false,
    });
  }
  if (typeof global.navigator !== "object" || global.navigator === null) {
    global.navigator = { userAgent: "nsh" };
  }
  try {
    global.navigator.gpu = gpu;
  } catch {
    Object.defineProperty(global.navigator, "gpu", { value: gpu, configurable: true });
  }
};
