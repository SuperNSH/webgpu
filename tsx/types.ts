// react-native-webgpu's own types (its src/types.ts, commit 06599f9): the
// canvas, native buffers, video, shared texture memory and fences, the
// drawElementImageToTexture arguments and Dawn toggles. Same names, members
// and optionality as the reference; what nsh-webgpu cannot back keeps its
// type so code written against the reference type checks (area: react-parity,
// docs/parity/react-parity.md).

import type { GPUCanvasContext } from "./api/GPUCanvasContext";

type SurfacePointer = bigint;

/** The native surface of a canvas, as `CanvasRef.getNativeSurface()` gives it. */
export interface NativeCanvas {
  /** The platform surface's pointer (CAMetalLayer, ANativeWindow, ...), `0n` when there is none. */
  surface: SurfacePointer;
  /** The drawing buffer's width. */
  width: number;
  /** The drawing buffer's height. */
  height: number;
  /** The view's layout width. */
  clientWidth: number;
  /** The view's layout height. */
  clientHeight: number;
  /** A no-op, so web renderers (Three.js) can treat the canvas as an HTMLCanvasElement. */
  addEventListener(type: string, listener: EventListener): void;
  /** A no-op, as `addEventListener`. */
  removeEventListener(type: string, listener: EventListener): void;
  /** A no-op, as `addEventListener`. */
  dispatchEvent(event: Event): void;
  /** A no-op, as `addEventListener`. */
  setPointerCapture(pointerId: number): void;
  /** A no-op, as `addEventListener`. */
  releasePointerCapture(pointerId: number): void;
}

/** A canvas context with react-native-webgpu's `present()`. */
export type RNCanvasContext = GPUCanvasContext & {
  /** Presents the current frame: call it after `queue.submit()`. */
  present: () => void;
};

/** What a `<Canvas ref>` holds. */
export interface CanvasRef {
  /** The canvas's context id, unique for the process. */
  getContextId: () => number;
  /** Its WebGPU context; `null` for another name. */
  getContext(contextName: "webgpu"): RNCanvasContext | null;
  /** Its native surface. */
  getNativeSurface: () => NativeCanvas;
}

/** A native buffer's pixel layout: single-plane BGRA, or biplanar Y + CbCr. */
export type NativeBufferPixelFormat = "bgra8" | "nv12";

/**
 * A native, GPU-shareable buffer: a video or camera frame, or a buffer to
 * render into. `handle` is its IOSurfaceRef / AHardwareBuffer* (`0n` for a
 * synthetic frame of `createTestVideoFrame`, which has no platform surface).
 */
export interface NativeBuffer {
  /** The raw native pointer, as a BigInt. */
  readonly handle: bigint;
  /** Its width in pixels. */
  readonly width: number;
  /** Its height in pixels. */
  readonly height: number;
  /** Its pixel layout. */
  readonly pixelFormat: NativeBufferPixelFormat;
  /** Android's acquire fence fd; `null` elsewhere and once released. */
  readonly acquireFenceFd: bigint | null;
  /** Drops the backing object now rather than when collected. */
  release(): void;
}

/** The color space of a YUV frame, as its producer reports it. */
export interface NativeBufferColorSpace {
  /** The YCbCr matrix. */
  matrix?: "identity" | "rec601" | "rec709" | "rec2020";
  /** The sample range. */
  range?: "identity" | "narrow" | "full";
  /** The RGB primaries. */
  primaries?: "srgb" | "rec709" | "rec601" | "rec2020" | "display-p3";
  /** The transfer function. */
  transfer?: "identity" | "srgb" | "display-p3" | "smpte-170m" | "hlg" | "pq" | "bt-1886";
}

/** A 2D size, as `{ width, height }` or `[width, height]`. */
export type GPUExtent2D = { width: number; height: number } | [number, number];

/** `GPUExternalTextureDescriptor` with a native buffer (or a shared-memory texture) as the source. */
export interface GPUNativeExternalTextureDescriptor extends Omit<GPUExternalTextureDescriptor, "source"> {
  /** The frame, or a texture of `GPUSharedTextureMemory.createTexture()`. */
  source: NativeBuffer | GPUTexture;
  /** Overrides members of the frame's own color space. */
  sourceColorSpace?: NativeBufferColorSpace;
  /** The origin of the part of the frame shown. */
  cropOrigin?: GPUOrigin2D;
  /** The size of the part of the frame shown. */
  cropSize?: GPUExtent2D;
}

/** A decoded video, modelled on HTMLMediaElement; frames are pulled with `copyLatestFrame()`. */
export interface VideoPlayer {
  /** The most recently decoded frame, `null` between frames. */
  copyLatestFrame(): NativeBuffer | null;
  /** Starts playing. */
  play(): void;
  /** Pauses. */
  pause(): void;
  /** Stops and releases the decoder. */
  release(): void;
  /** Whether it is paused. */
  readonly paused: boolean;
  /** The position in seconds; assign it to seek. */
  currentTime: number;
  /** The length in seconds, 0 until the metadata loaded. */
  readonly duration: number;
  /** 0 to 1. */
  volume: number;
  /** Restart at the end; `true` by default. */
  loop: boolean;
  /** The coded width, 0 until the metadata loaded. */
  readonly videoWidth: number;
  /** The coded height, 0 until the metadata loaded. */
  readonly videoHeight: number;
  /** The clockwise rotation to display the frames with. */
  readonly rotation: 0 | 90 | 180 | 270;
  /** The nominal frame rate, 0 if unknown. */
  readonly frameRate: number;
}

/** The source of `GPUQueue.copyExternalImageToTexture()` for a native frame. */
export interface GPUCopyExternalNativeFrameSourceInfo {
  /** The frame. */
  source: NativeBuffer;
  /** Where to copy from, in the upright frame. */
  origin?: GPUOrigin2D;
  /** Whether to flip vertically. */
  flipY?: boolean;
  /** The clockwise rotation to apply. */
  rotation?: 0 | 90 | 180 | 270;
  /** Whether to mirror horizontally. */
  mirrored?: boolean;
}

/** `createVideoPlayer(source, options)`'s options. */
export interface CreateVideoPlayerOptions {
  /** The frames' layout on Apple platforms (`"bgra8"` by default). */
  pixelFormat?: NativeBufferPixelFormat;
}

/** A view to draw with `drawElementImageToTexture`: a ref object, an instance or a view tag. */
export type GPUDrawElementImageSourceView = number | { current: unknown } | object;

/** The source of `drawElementImageToTexture`: a view and, optionally, a rectangle of it in points. */
export interface GPUDrawElementImageSource {
  /** The view. */
  source: GPUDrawElementImageSourceView;
  /** The rectangle's left. */
  sourceX?: number;
  /** The rectangle's top. */
  sourceY?: number;
  /** The rectangle's width. */
  sourceWidth?: number;
  /** The rectangle's height. */
  sourceHeight?: number;
  /** iOS: flush pending layout before drawing (`true`). */
  afterScreenUpdates?: boolean;
}

/** The destination of `drawElementImageToTexture`. */
export interface GPUDrawElementImageDestination extends GPUCopyExternalImageDestInfo {
  /** The pixel size of the region drawn into. */
  size?: GPUExtent3DStrict;
}

/** `GPUDevice.importSharedTextureMemory()`'s descriptor: a native buffer or a raw handle. */
export type GPUSharedTextureMemoryDescriptor = {
  label?: string;
} & (
  | {
      source: NativeBuffer;
      handle?: undefined;
    }
  | {
      handle: bigint;
      source?: undefined;
    }
);

/** The kind of native fence a `GPUSharedFence` wraps. */
export type GPUSharedFenceType = "mtl-shared-event" | "sync-fd" | "vk-semaphore-opaque-fd";

/** `GPUDevice.importSharedFence()`'s descriptor. */
export interface GPUSharedFenceDescriptor {
  /** The fence kind. */
  type: GPUSharedFenceType;
  /** The raw native handle, as a BigInt. */
  handle: bigint;
  /** Its label. */
  label?: string;
}

/** What `GPUSharedFence.export()` gives. */
export interface GPUSharedFenceExportInfo {
  /** The fence kind. */
  type: GPUSharedFenceType;
  /** The raw native handle, owned by the caller. */
  handle: bigint;
}

/** A native GPU synchronization primitive shared across queues and APIs. */
export interface GPUSharedFence {
  readonly __brand: "GPUSharedFence";
  /** Its label. */
  label: string;
  /** Exports its native handle. */
  export(): GPUSharedFenceExportInfo;
  /** Releases it now. */
  destroy(): void;
}

/** A fence and the value to wait for. */
export interface GPUSharedFenceState {
  /** The fence. */
  fence: GPUSharedFence;
  /** The value it is signalled at. */
  signaledValue: bigint;
}

/** What `GPUSharedTextureMemory.endAccess()` gives. */
export interface GPUSharedTextureMemoryEndAccessState {
  /** Whether the surface holds meaningful pixels. */
  initialized: boolean;
  /** The fences signalled once the access's GPU work completes. */
  fences: GPUSharedFenceState[];
}

/** Dawn's device toggles (`GPUDeviceDescriptor.dawnToggles`); wgpu has none, so they are accepted and ignored. */
export interface GPUDawnTogglesDescriptor {
  /** Toggles to turn on. */
  enabledToggles?: string[];
  /** Toggles to turn off. */
  disabledToggles?: string[];
}

/** Shared GPU memory backed by a native surface. */
export interface GPUSharedTextureMemory {
  readonly __brand: "GPUSharedTextureMemory";
  /** Its label. */
  label: string;
  /** A texture aliasing the surface's pixels. */
  createTexture(descriptor?: GPUTextureDescriptor): GPUTexture;
  /** Begins an access to `texture`. */
  beginAccess(texture: GPUTexture, initialized: boolean, fences?: GPUSharedFenceState[]): void;
  /** Ends the access and gives its fences. */
  endAccess(texture: GPUTexture): GPUSharedTextureMemoryEndAccessState;
  /** Releases the memory now. */
  destroy(): void;
}

/** `createNativeBuffer()`'s options. The buffer holds premultiplied pixels. */
export interface CreateNativeBufferOptions {
  /** `"rgba8unorm"` on Android, `"bgra8unorm"` on Apple platforms by default. */
  format?: "rgba8unorm" | "bgra8unorm" | "rgba16float";
  /** The primaries of the values (`"srgb"`). */
  colorSpace?: PredefinedColorSpace;
}
