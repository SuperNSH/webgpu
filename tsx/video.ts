// Video and native buffers (react-native-webgpu's src/video.ts), area:
// react-parity. createTestVideoFrame is implemented: its frame is pure data
// (the reference's pattern, byte for byte), held in memory instead of an
// IOSurface / AHardwareBuffer. The other three need a platform media or
// buffer API (AVPlayer / MediaCodec, CVPixelBuffer / AHardwareBuffer,
// IOSurface) and the shared-texture-memory import that wgpu does not have:
// they throw ERR_WEBGPU_UNSUPPORTED (docs/parity/react-parity.md).

import { unsupportedError } from "./internal/native";
import type {
  CreateNativeBufferOptions,
  CreateVideoPlayerOptions,
  NativeBuffer,
  NativeBufferPixelFormat,
  VideoPlayer,
} from "./types";

/** The reference's NV12 test colours: BT.709 limited range (cpp/rnwgpu/PlatformContext.h). */
const RED_Y = 63;
const RED_CB = 102;
const RED_CR = 240;
const GREEN_Y = 173;
const GREEN_CB = 42;
const GREEN_CR = 26;

/**
 * A frame held in memory: the bytes of its planes, tightly packed. bgra8:
 * one plane, 4 bytes a pixel in B, G, R, A order. nv12: the Y plane (1 byte a
 * pixel), then the CbCr plane (2 bytes per 2x2 block, Cb then Cr).
 */
class MemoryNativeBuffer implements NativeBuffer {
  readonly handle = 0n;
  readonly width: number;
  readonly height: number;
  readonly pixelFormat: NativeBufferPixelFormat;
  readonly acquireFenceFd: bigint | null = null;
  #planes: Uint8Array[] | null;

  constructor(width: number, height: number, pixelFormat: NativeBufferPixelFormat, planes: Uint8Array[]) {
    this.width = width;
    this.height = height;
    this.pixelFormat = pixelFormat;
    this.#planes = planes;
  }

  /**
   * The bytes of plane `index` (bgra8: 0; nv12: 0 is Y, 1 is CbCr), for
   * `queue.writeTexture()`; throws once released. An nsh-webgpu extension:
   * the frame has no platform surface to import.
   */
  plane(index = 0): Uint8Array {
    if (this.#planes === null) {
      throw new Error("NativeBuffer: the frame was released");
    }
    const plane = this.#planes[index];
    if (plane === undefined) {
      throw new RangeError(`NativeBuffer: no plane ${index} in a ${this.pixelFormat} frame`);
    }
    return plane;
  }

  release(): void {
    this.#planes = null;
  }

  get [Symbol.toStringTag](): string {
    return "NativeBuffer";
  }
}

function checkSize(name: string, width: number, height: number): void {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 || width > 16384 || height > 16384) {
    throw new RangeError(`${name}: the width and height must be integers from 1 to 16384, got ${width}x${height}`);
  }
}

function bgraTestPattern(width: number, height: number): Uint8Array {
  const bytes = new Uint8Array(width * height * 4);
  const wMax = Math.max(width - 1, 1);
  const hMax = Math.max(height - 1, 1);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const at = (y * width + x) * 4;
      bytes[at] = (x + y) & 0x20 ? 220 : 30; // B: diagonal stripes
      bytes[at + 1] = Math.floor((y * 255) / hMax); // G: vertical gradient
      bytes[at + 2] = Math.floor((x * 255) / wMax); // R: horizontal gradient
      bytes[at + 3] = 0xff;
    }
  }
  return bytes;
}

function nv12TestPattern(width: number, height: number): Uint8Array[] {
  const luma = new Uint8Array(width * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      luma[y * width + x] = x < Math.floor(width / 2) ? RED_Y : GREEN_Y;
    }
  }
  const chromaWidth = Math.ceil(width / 2);
  const chromaHeight = Math.ceil(height / 2);
  const chroma = new Uint8Array(chromaWidth * chromaHeight * 2);
  for (let y = 0; y < chromaHeight; y++) {
    for (let x = 0; x < chromaWidth; x++) {
      const red = x < Math.floor(chromaWidth / 2);
      const at = (y * chromaWidth + x) * 2;
      chroma[at] = red ? RED_CB : GREEN_CB;
      chroma[at + 1] = red ? RED_CR : GREEN_CR;
    }
  }
  return [luma, chroma];
}

/**
 * Decodes a video into native frames. Needs a platform decoder (AVPlayer,
 * MediaCodec) and zero-copy frame import: unsupported in nsh-webgpu.
 */
export const createVideoPlayer = (source: string, options?: CreateVideoPlayerOptions): VideoPlayer => {
  void [source, options];
  throw unsupportedError(
    "createVideoPlayer is unavailable: nsh-webgpu has no platform video decoder (AVPlayer, MediaCodec) and wgpu cannot import its frames (IOSurface, AHardwareBuffer) as textures",
  );
};

/**
 * Wraps another library's CVPixelBufferRef / AHardwareBuffer* as a
 * NativeBuffer: unsupported in nsh-webgpu (wgpu cannot import it).
 */
export const createNativeBufferFromPointer = (pointer: bigint): NativeBuffer => {
  void pointer;
  throw unsupportedError(
    "createNativeBufferFromPointer is unavailable: wgpu has no shared texture memory to import a CVPixelBufferRef or AHardwareBuffer*",
  );
};

/**
 * A frame filled with the reference's test pattern (a red/green gradient
 * with diagonal stripes in the blue channel, bgra8), for examples and
 * tests that need a frame without a camera or a video. The frame lives in
 * memory (`handle` is `0n`); its bytes are in `plane(i)`.
 */
export const createTestVideoFrame = (width: number, height: number): NativeBuffer =>
  testVideoFrame(width, height, "bgra8");

/**
 * What the reference's `RNWebGPU.createTestVideoFrame(width, height,
 * pixelFormat)` makes (not a package export, as in the reference): with
 * `"nv12"`, a BT.709 limited-range frame, red on the left half and green
 * on the right.
 */
export const testVideoFrame = (
  width: number,
  height: number,
  pixelFormat: NativeBufferPixelFormat = "bgra8",
): NativeBuffer & { plane(index?: number): Uint8Array } => {
  checkSize("createTestVideoFrame", width, height);
  if (pixelFormat === "nv12") {
    return new MemoryNativeBuffer(width, height, "nv12", nv12TestPattern(width, height));
  }
  return new MemoryNativeBuffer(width, height, "bgra8", [bgraTestPattern(width, height)]);
};

/**
 * An empty IOSurface / AHardwareBuffer to render into and show in a
 * NativeBufferView: unsupported in nsh-webgpu (wgpu has no shared texture
 * memory to render into it).
 */
export const createNativeBuffer = (width: number, height: number, options?: CreateNativeBufferOptions): NativeBuffer => {
  void [width, height, options];
  throw unsupportedError(
    "createNativeBuffer is unavailable: wgpu has no shared texture memory to render into an IOSurface or AHardwareBuffer",
  );
};
