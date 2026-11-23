// The DOM names @webgpu/types (./webgpu.d.ts) refers to that NSH's globals
// do not declare. NSH has no DOM: these are empty or minimal, enough for
// the WebGPU declarations to type check. A GPUCanvasContext's `canvas` is
// nsh-webgpu's canvas object, which the canvas area types fully.

interface EventListenerOptions {
  capture?: boolean;
}
interface HTMLCanvasElement {
  width: number;
  height: number;
}
interface HTMLImageElement {}
interface OffscreenCanvas {
  width: number;
  height: number;
}
interface ImageBitmap {
  readonly width: number;
  readonly height: number;
  close(): void;
}
interface ImageData {
  readonly width: number;
  readonly height: number;
  readonly data: Uint8ClampedArray;
}
interface VideoFrame {}
type PredefinedColorSpace = "srgb" | "display-p3";
type AllowSharedBufferSource = ArrayBuffer | ArrayBufferView;
interface Navigator {}
interface WorkerNavigator {}
declare var navigator: Navigator;
interface ImageBitmapOptions {
  colorSpaceConversion?: "default" | "none";
  imageOrientation?: "from-image" | "flipY" | "none";
  premultiplyAlpha?: "default" | "none" | "premultiply";
  resizeHeight?: number;
  resizeQuality?: "high" | "low" | "medium" | "pixelated";
  resizeWidth?: number;
}
/** The DOM's `EventListener` (NSH declares `EventListenerOrEventListenerObject` only). */
type EventListener = (event: Event) => void;

/** What `createImageBitmap` reads besides encoded bytes (react-native-webgpu's sources). */
type ImageBitmapSource = ImageBitmap | ImageData | { arrayBuffer(): Promise<ArrayBuffer> };

/**
 * `createImageBitmap`, as react-native-webgpu extends it: encoded image
 * bytes (PNG, JPEG, WebP) as an `ArrayBuffer` or a view, besides the DOM's
 * sources. nsh-webgpu installs it on `globalThis` (`installWebGPU()`).
 */
declare function createImageBitmap(
  image: ArrayBuffer | ArrayBufferView | ImageBitmapSource,
  options?: ImageBitmapOptions,
): Promise<ImageBitmap>;
declare function createImageBitmap(
  image: ArrayBuffer | ArrayBufferView | ImageBitmapSource,
  sx: number,
  sy: number,
  sw: number,
  sh: number,
  options?: ImageBitmapOptions,
): Promise<ImageBitmap>;
