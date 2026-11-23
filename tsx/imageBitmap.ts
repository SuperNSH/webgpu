// createImageBitmap and ImageBitmap, as react-native-webgpu extends them:
// encoded image bytes (an ArrayBuffer or a view, the view's range only)
// are decoded natively (src/api/image_bitmap.rs: PNG, JPEG, WebP), and the
// pixels stay native, for copyExternalImageToTexture. Also an ImageData
// (`{ data, width, height }`, straight RGBA8), another ImageBitmap, or a
// Blob-like object with `arrayBuffer()`. Both overloads: (image, options)
// and (image, sx, sy, sw, sh, options). install.ts makes createImageBitmap
// the global when the runtime has none.

import { callAsync, call, WgpuImageBitmap } from "./internal/native";
import { INTERNAL, internal, track } from "./internal/object";

/** What the native side reports of a new bitmap. */
interface Made {
  id: number;
  width: number;
  height: number;
  premultiplied: boolean;
}

/** Decoded pixels, ready for `queue.copyExternalImageToTexture`. */
export class ImageBitmap {
  #id: number | null;
  #width: number;
  #height: number;
  readonly #premultiplied: boolean;

  constructor(token: unknown, made: Made) {
    internal(token);
    this.#id = made.id;
    this.#width = made.width;
    this.#height = made.height;
    this.#premultiplied = made.premultiplied;
    track(this, made.id);
  }

  /** Width in pixels; 0 once closed. */
  get width(): number {
    return this.#width;
  }

  /** Height in pixels; 0 once closed. */
  get height(): number {
    return this.#height;
  }

  /** Releases the pixels: the dimensions become 0. Closing twice does nothing. */
  close(): void {
    if (this.#id === null) return;
    WgpuImageBitmap.close(this.#id);
    this.#id = null;
    this.#width = 0;
    this.#height = 0;
  }

  /** The native pixels' id, `null` once closed (nsh-webgpu's own use). */
  static handleOf(bitmap: ImageBitmap): number | null {
    return bitmap.#id;
  }

  /** Whether the pixels are premultiplied by alpha (nsh-webgpu's own use). */
  static premultipliedOf(bitmap: ImageBitmap): boolean {
    return bitmap.#premultiplied;
  }

  get [Symbol.toStringTag](): string {
    return "ImageBitmap";
  }
}

/** An ImageData-like object: straight RGBA8 `data`, `width` x `height`. */
interface ImageDataLike {
  data: Uint8ClampedArray | Uint8Array;
  width: number;
  height: number;
}

function isImageData(value: unknown): value is ImageDataLike {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Partial<ImageDataLike>;
  return ArrayBuffer.isView(v.data) && typeof v.width === "number" && typeof v.height === "number";
}

/** The bytes of a real ArrayBuffer or view (its range only); a TypeError for anything else. */
function bufferSource(value: unknown): Uint8Array | null {
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer as ArrayBuffer, value.byteOffset, value.byteLength);
  return null;
}

/** Straight copies of premultiplied RGBA8 `data`, react-native-webgpu's rounding. */
function unpremultiply(data: Uint8Array): Uint8Array {
  const out = new Uint8Array(data);
  for (let i = 0; i + 3 < out.length; i += 4) {
    const a = out[i + 3];
    for (let c = 0; c < 3; c++) {
      out[i + c] = a === 0 ? 0 : Math.min(255, Math.floor((out[i + c] * 255 + (a >> 1)) / a));
    }
  }
  return out;
}

/** Makes an ImageBitmap of `image` with `options` (and crop rectangle `crop`). */
async function make(image: unknown, options: ImageBitmapOptions | undefined, crop: number[] | undefined): Promise<ImageBitmap> {
  const nativeOptions = { ...(options ?? {}), ...(crop ? { crop } : {}) };
  if (image instanceof ImageBitmap) {
    const id = ImageBitmap.handleOf(image);
    if (id === null) throw new DOMException("createImageBitmap: the ImageBitmap is closed", "InvalidStateError");
    let pixels: Uint8Array = new Uint8Array(call(() => WgpuImageBitmap.pixels(id)));
    if (ImageBitmap.premultipliedOf(image)) pixels = unpremultiply(pixels);
    const made = call(() => WgpuImageBitmap.fromPixels(pixels, image.width, image.height, nativeOptions)) as Made;
    return new ImageBitmap(INTERNAL, made);
  }
  if (isImageData(image)) {
    const bytes = new Uint8Array(image.data.buffer as ArrayBuffer, image.data.byteOffset, image.data.byteLength);
    const made = call(() => WgpuImageBitmap.fromPixels(bytes, image.width, image.height, nativeOptions)) as Made;
    return new ImageBitmap(INTERNAL, made);
  }
  let bytes: Uint8Array | null = bufferSource(image);
  if (bytes === null && typeof (image as { arrayBuffer?: unknown })?.arrayBuffer === "function") {
    bytes = new Uint8Array(await (image as { arrayBuffer(): Promise<ArrayBuffer> }).arrayBuffer());
  }
  if (bytes === null) {
    throw new TypeError(
      "createImageBitmap: the source must be an ArrayBuffer, an ArrayBufferView, an ImageData, an ImageBitmap or a Blob",
    );
  }
  const made = (await callAsync(() => WgpuImageBitmap.decode(bytes, nativeOptions))) as Made;
  return new ImageBitmap(INTERNAL, made);
}

/**
 * Decodes `image` into an ImageBitmap: `(image, options?)` or
 * `(image, sx, sy, sw, sh, options?)`. Rejects with a TypeError for a
 * source it cannot read and an InvalidStateError for bytes it cannot
 * decode.
 */
export function createImageBitmap(
  image: ArrayBuffer | ArrayBufferView | ImageBitmapSource,
  options?: ImageBitmapOptions,
): Promise<ImageBitmap>;
export function createImageBitmap(
  image: ArrayBuffer | ArrayBufferView | ImageBitmapSource,
  sx: number,
  sy: number,
  sw: number,
  sh: number,
  options?: ImageBitmapOptions,
): Promise<ImageBitmap>;
export function createImageBitmap(
  image: ArrayBuffer | ArrayBufferView | ImageBitmapSource,
  a?: ImageBitmapOptions | number,
  sy?: number,
  sw?: number,
  sh?: number,
  options?: ImageBitmapOptions,
): Promise<ImageBitmap> {
  return (async () => {
    if (typeof a === "number") {
      const rect = [a, sy, sw, sh].map((v, i) => {
        const n = Math.trunc(Number(v));
        if (!Number.isFinite(n)) throw new TypeError(`createImageBitmap: crop argument ${i} is not a number`);
        return n;
      });
      if (rect[2] === 0 || rect[3] === 0) {
        throw new RangeError("createImageBitmap: the crop width and height must not be 0");
      }
      return make(image, options, rect);
    }
    return make(image, a, undefined);
  })();
}

/** The sources createImageBitmap reads (beyond the bytes overload). */
export type ImageBitmapSource = ImageBitmap | ImageDataLike | { arrayBuffer(): Promise<ArrayBuffer> };
