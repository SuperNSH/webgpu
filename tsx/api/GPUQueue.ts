// GPUQueue (`device.queue`): submit, writeBuffer, writeTexture,
// onSubmittedWorkDone, copyExternalImageToTexture (src/api/queue.rs).

import { ImageBitmap } from "../imageBitmap";
import { call, callAsync, unsupported, unsupportedError, Wgpu, WgpuImageBitmap, WgpuQueue } from "../internal/native";
import { HANDLE, handleOf, internal } from "../internal/object";
import { enforceRange, U64_MAX } from "../internal/range";
import { wire } from "../internal/wire";
import type { SpecGPUQueue } from "../types/spec";
import type {
  GPUCopyExternalNativeFrameSourceInfo,
  GPUDrawElementImageDestination,
  GPUDrawElementImageSource,
} from "../types";
import type { GPUBuffer } from "./GPUBuffer";
import type { GPUCommandBuffer } from "./GPUCommandBuffer";
import type { GPUDevice } from "./GPUDevice";

function u64(value: unknown, name: string): number {
  return enforceRange(value, name, U64_MAX);
}

/**
 * `data` as bytes: its `dataOffset` and `size` counted in its elements
 * (bytes of an ArrayBuffer or DataView, elements of a typed array). A
 * TypeError for anything but a real ArrayBuffer or view (a look-alike
 * object never reaches native code), an OperationError for a range outside
 * the data.
 */
export function bytesOf(
  data: GPUAllowSharedBufferSource,
  dataOffset: number | undefined,
  size: number | undefined,
  what = "writeBuffer",
): Uint8Array {
  const view = ArrayBuffer.isView(data);
  if (!view && !(data instanceof ArrayBuffer) && !(typeof SharedArrayBuffer === "function" && data instanceof SharedArrayBuffer)) {
    throw new TypeError(`${what}: data must be an ArrayBuffer or an ArrayBufferView`);
  }
  const buffer = view ? data.buffer : (data as ArrayBuffer);
  const base = view ? data.byteOffset : 0;
  const byteLength = view ? data.byteLength : (data as ArrayBuffer).byteLength;
  const elementSize =
    view && "BYTES_PER_ELEMENT" in data ? (data as unknown as { BYTES_PER_ELEMENT: number }).BYTES_PER_ELEMENT : 1;
  const elements = byteLength / elementSize;
  const offset = dataOffset === undefined ? 0 : u64(dataOffset, "dataOffset");
  if (offset > elements) {
    throw new DOMException(`${what}: dataOffset ${offset} is past data's ${elements} elements`, "OperationError");
  }
  const count = size === undefined ? elements - offset : u64(size, "size");
  if (offset + count > elements) {
    throw new DOMException(
      `${what}: [${offset}, ${offset + count}) is outside data's ${elements} elements`,
      "OperationError",
    );
  }
  return new Uint8Array(buffer as ArrayBuffer, base + offset * elementSize, count * elementSize);
}

/** A device's queue: where work is submitted. */
export class GPUQueue implements SpecGPUQueue {
  declare readonly __brand: "GPUQueue";
  /** The label, `""` by default. */
  label: string;
  readonly #device: GPUDevice;

  constructor(token: unknown, device: GPUDevice, label: string | undefined) {
    internal(token);
    this.#device = device;
    this.label = label ?? "";
  }

  /** Runs `commandBuffers`, in order. A command buffer runs once. */
  submit(commandBuffers: Iterable<GPUCommandBuffer>): undefined {
    const ids = Array.from(commandBuffers, (cb) => handleOf(cb, "GPUCommandBuffer"));
    call(() => WgpuQueue.submit(this.#device[HANDLE], ids));
    return undefined;
  }

  /** Settles once the work submitted so far is done. */
  async onSubmittedWorkDone(): Promise<undefined> {
    const ticket = call(() => WgpuQueue.onSubmittedWorkDone(this.#device[HANDLE]));
    await callAsync(() => Wgpu.wait(ticket));
    this.#device.drainErrors();
    return undefined;
  }

  /**
   * Writes `data` into `buffer` at `bufferOffset`: from `dataOffset`,
   * `size` elements (bytes of an ArrayBuffer, elements of a typed array).
   * The size in bytes must be a multiple of 4 (an OperationError).
   */
  writeBuffer(
    buffer: GPUBuffer,
    bufferOffset: number,
    data: GPUAllowSharedBufferSource,
    dataOffset?: number,
    size?: number,
  ): undefined {
    const target = handleOf(buffer, "GPUBuffer");
    const offset = u64(bufferOffset, "bufferOffset");
    const bytes = bytesOf(data, dataOffset, size);
    if (bytes.byteLength % 4 !== 0) {
      throw new DOMException(`writeBuffer: ${bytes.byteLength} bytes is not a multiple of 4`, "OperationError");
    }
    call(() => WgpuQueue.writeBuffer(this.#device[HANDLE], target, offset, bytes));
    return undefined;
  }

  /** Writes `data` into a texture; `dataLayout.offset` counts bytes from the view's start. */
  writeTexture(
    destination: GPUTexelCopyTextureInfo,
    data: GPUAllowSharedBufferSource,
    dataLayout: GPUTexelCopyBufferLayout,
    size: GPUExtent3DStrict,
  ): undefined {
    const bytes = bytesOf(data, undefined, undefined, "writeTexture");
    // Whole bytes from here on: a typed array's elements are not the unit.
    const view = new Uint8Array(bytes.buffer as ArrayBuffer, bytes.byteOffset, bytes.byteLength);
    call(() =>
      WgpuQueue.writeTexture(this.#device[HANDLE], wire(destination), view, wire(dataLayout), wire(size)),
    );
    return undefined;
  }

  /**
   * react-native-webgpu's view-to-texture extension: rejects with
   * ERR_WEBGPU_UNSUPPORTED, NSH has no hook that draws a view subtree into
   * pixels (docs/parity.md).
   */
  drawElementImageToTexture(
    _source: GPUDrawElementImageSource,
    _destination: GPUDrawElementImageDestination,
  ): Promise<void> {
    return Promise.reject(
      unsupportedError("GPUQueue.drawElementImageToTexture is not supported by nsh-webgpu (see docs/parity.md)"),
    );
  }

  /**
   * Copies an image into a texture: an ImageBitmap (createImageBitmap) or
   * an ImageData (`{ data, width, height }`). A region outside the image
   * is an OperationError; a closed ImageBitmap an InvalidStateError.
   * react-native-webgpu's NativeBuffer source and the DOM's video and
   * canvas sources are ERR_WEBGPU_UNSUPPORTED (docs/parity.md).
   */
  copyExternalImageToTexture(
    source: GPUCopyExternalImageSourceInfo | GPUCopyExternalNativeFrameSourceInfo,
    destination: GPUCopyExternalImageDestInfo,
    copySize: GPUExtent3DStrict,
  ): undefined {
    const image = source.source as unknown;
    let bitmap: number;
    let temporary = false;
    if (image instanceof ImageBitmap) {
      const id = ImageBitmap.handleOf(image);
      if (id === null) {
        throw new DOMException("copyExternalImageToTexture: the ImageBitmap is closed", "InvalidStateError");
      }
      bitmap = id;
    } else if (
      typeof image === "object" &&
      image !== null &&
      ArrayBuffer.isView((image as ImageData).data) &&
      typeof (image as ImageData).width === "number"
    ) {
      const data = (image as ImageData).data;
      const pixels = new Uint8Array(data.buffer as ArrayBuffer, data.byteOffset, data.byteLength);
      const made = call(() =>
        WgpuImageBitmap.fromPixels(pixels, (image as ImageData).width, (image as ImageData).height, {
          premultiplyAlpha: "none",
        }),
      ) as { id: number };
      bitmap = made.id;
      temporary = true;
    } else {
      return unsupported("copyExternalImageToTexture from this source (only ImageBitmap and ImageData)");
    }
    try {
      const info = { origin: wire(source.origin), flipY: source.flipY, bitmap };
      call(() =>
        WgpuQueue.copyExternalImageToTexture(this.#device[HANDLE], wire(info), wire(destination), wire(copySize)),
      );
    } finally {
      if (temporary) WgpuImageBitmap.close(bitmap);
    }
    return undefined;
  }

  get [Symbol.toStringTag](): string {
    return "GPUQueue";
  }
}
