// GPUOffscreenCanvas (react-native-webgpu's src/Offscreen.ts), area:
// react-parity. Pure WebGPU, as in the reference: the context is one texture
// of the preferred canvas format (RENDER_ATTACHMENT | COPY_SRC |
// TEXTURE_BINDING), made at configure() with the canvas's size, and
// getImageData() reads it back. One deliberate difference: the reference
// copies rows of `width * 4` bytes, which WebGPU only accepts when that is
// a multiple of 256 (width a multiple of 64); here rows are copied padded to
// 256 and the padding is dropped, so any width reads back. For the widths
// the reference accepts, the result is the same.

/** What `getImageData()` resolves with: RGBA-sized pixels (4 bytes each), row by row. */
interface GPUOffscreenCanvasImageData {
  data: number[];
  width: number;
  height: number;
  format: GPUTextureFormat;
}

/** WebGPU's row alignment for texture-to-buffer copies. */
const ROW_ALIGNMENT = 256;

/** A canvas with no view, like the web's OffscreenCanvas: render into it, then read it back. */
export class GPUOffscreenCanvas {
  /** Its width in pixels; read at `configure()`. */
  width: number;
  /** Its height in pixels; read at `configure()`. */
  height: number;

  private context: GPUOffscreenCanvasContext;

  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
    this.context = new GPUOffscreenCanvasContext(this);
  }

  /** Its WebGPU context (always the same one); `null` for another id. */
  getContext(contextId: "webgpu"): GPUOffscreenCanvasContext | null {
    if (contextId === "webgpu") {
      return this.context;
    }
    return null;
  }

  /** Copies the texture into a buffer, maps it and resolves with its bytes. */
  getImageData(): Promise<GPUOffscreenCanvasImageData> {
    const device = this.context.getDevice();
    const texture = this.context.getTexture();
    const commandEncoder = device.createCommandEncoder();
    const bytesPerRow = this.width * 4;
    const paddedBytesPerRow = Math.ceil(bytesPerRow / ROW_ALIGNMENT) * ROW_ALIGNMENT;
    const buffer = device.createBuffer({
      size: paddedBytesPerRow * this.height,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    commandEncoder.copyTextureToBuffer(
      { texture: texture },
      { buffer: buffer, bytesPerRow: paddedBytesPerRow },
      [this.width, this.height],
    );
    device.queue.submit([commandEncoder.finish()]);

    return buffer.mapAsync(GPUMapMode.READ).then(() => {
      const mapped = new Uint8Array(buffer.getMappedRange());
      const data: number[] = new Array<number>(bytesPerRow * this.height);
      for (let y = 0; y < this.height; y++) {
        const row = y * paddedBytesPerRow;
        for (let x = 0; x < bytesPerRow; x++) {
          data[y * bytesPerRow + x] = mapped[row + x];
        }
      }
      buffer.unmap();
      buffer.destroy();
      return {
        data,
        width: this.width,
        height: this.height,
        format: navigator.gpu.getPreferredCanvasFormat(),
      };
    });
  }
}

/** The context of a `GPUOffscreenCanvas`: one texture, the same every frame. Not exported, as in the reference. */
class GPUOffscreenCanvasContext implements GPUCanvasContext {
  readonly __brand = "GPUCanvasContext" as const;

  private textureFormat: GPUTextureFormat;
  private texture: GPUTexture | null = null;
  private device: GPUDevice | null = null;

  constructor(readonly canvas: GPUOffscreenCanvas) {
    this.textureFormat = navigator.gpu.getPreferredCanvasFormat();
  }

  /** Throws, as the reference's does. */
  getConfiguration(): GPUCanvasConfigurationOut | null {
    throw new Error("Method not implemented.");
  }

  /** Nothing to present: read back with `getImageData()`. */
  present(): void {}

  /** The configured device; throws before `configure()`. */
  getDevice(): GPUDevice {
    if (!this.device) {
      throw new Error("Device is not configured.");
    }
    return this.device;
  }

  /** The texture; throws before `configure()`. */
  getTexture(): GPUTexture {
    if (!this.texture) {
      throw new Error("Texture is not configured");
    }
    return this.texture;
  }

  /** Makes the texture with the canvas's size. Like the reference, the format is always the preferred canvas format. */
  configure(config: GPUCanvasConfiguration): undefined {
    this.device = config.device;
    this.texture = config.device.createTexture({
      size: [this.canvas.width, this.canvas.height],
      format: this.textureFormat,
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC | GPUTextureUsage.TEXTURE_BINDING,
    });
    return undefined;
  }

  /** Destroys the texture. */
  unconfigure(): undefined {
    if (this.texture) {
      this.texture.destroy();
    }
    return undefined;
  }

  /** The texture, the same every call. */
  getCurrentTexture(): GPUTexture {
    if (!this.texture) {
      throw new Error("Texture is not configured");
    }
    return this.texture;
  }
}

