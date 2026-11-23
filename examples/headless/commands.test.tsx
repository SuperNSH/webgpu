// The commands area through the TSX classes, in the test runner: encoders
// and passes, render bundles, the queue's writes (element units, look-alike
// objects, byteOffset), createImageBitmap and copyExternalImageToTexture,
// GPUExternalTexture. Pixel comparisons against the reference's snapshots
// are cargo tests (tests/commands); here the TSX layer:
// argument conversion, overloads, errors, and the readbacks they lead to.

import { Buffer } from "buffer";
import { describe, expect, test } from "nsh/test";
import {
  GPUBufferUsage,
  GPUCommandBuffer as CommandBufferClass,
  GPUCommandEncoder as CommandEncoderClass,
  GPUComputePassEncoder as ComputePassClass,
  GPUExternalTexture as ExternalTextureClass,
  GPUMapMode,
  GPURenderBundle as RenderBundleClass,
  GPURenderBundleEncoder as RenderBundleEncoderClass,
  GPURenderPassEncoder as RenderPassClass,
  GPUTextureUsage,
} from "nsh-webgpu";

async function device(): Promise<GPUDevice> {
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("No adapter");
  return adapter.requestDevice();
}

/** `buffer`'s bytes, copied to a mappable buffer. */
async function readBack(d: GPUDevice, buffer: GPUBuffer, size: number): Promise<Uint8Array> {
  const read = d.createBuffer({ size, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  const encoder = d.createCommandEncoder();
  encoder.copyBufferToBuffer(buffer, 0, read, 0, size);
  d.queue.submit([encoder.finish()]);
  await read.mapAsync(GPUMapMode.READ);
  const bytes = new Uint8Array(read.getMappedRange().slice(0));
  read.unmap();
  return bytes;
}

/** A texture's `width` x `height` RGBA8 texels, rows packed. */
async function readTexture(d: GPUDevice, texture: GPUTexture, width: number, height: number): Promise<number[]> {
  const bytesPerRow = 256;
  const out = d.createBuffer({ size: bytesPerRow * height, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  const encoder = d.createCommandEncoder();
  encoder.copyTextureToBuffer({ texture }, { buffer: out, bytesPerRow }, [width, height]);
  d.queue.submit([encoder.finish()]);
  await out.mapAsync(GPUMapMode.READ);
  const mapped = new Uint8Array(out.getMappedRange());
  const data: number[] = [];
  for (let y = 0; y < height; y++) data.push(...mapped.subarray(y * bytesPerRow, y * bytesPerRow + width * 4));
  out.unmap();
  return data;
}

// The reference's 32 x 32 fixtures (snapshots/commands/assets), inline: the
// headless app has no filesystem grant.
const ASSETS: Record<string, string> = {
  "opaque-gradient.png": "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAhElEQVR4AcXBsW0EQRADMBpQoO2/2JvsXYbIP36/OnWeT506dZ5PnTp16jyfOnXqPJ86deo8nzp16tR5PnXq1Hk+depETUVNxTMVNRU1FTUVz1TUVNRU1FTUVDxTUVNRU1FT8UxFTUVNRU3FMxU1FTUVNRU1Fc9U1FTUVNRUPFNRU1FT/wsfInvd8GcEAAAAAElFTkSuQmCC",
  "alpha-gradient.png": "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAk0lEQVR4AcXBsW0EQQwAMRpQMNt/safsXYZI1/70Kyvr+WRlZT2frKysrOeTlZX1fLKysp5PVlZW1vPJysp6PllZIzk0nufQSA6N5NBIDo3nOTSSQyM5NJJDIzk0nufQSA6N5NBIDo3nOTSSQyM5NJJD43kOjeTQSA6N5NBIDo3nOTSSQyM5NJJD43kOjeTQSA79A6M8Inck27PMAAAAAElFTkSuQmCC",
};

async function asset(name: string): Promise<Uint8Array> {
  return new Uint8Array(Buffer.from(ASSETS[name], "base64"));
}

/** What `fn` throws: its name (TypeError, OperationError, ...), or "ok". */
function thrown(fn: () => unknown): string {
  try {
    fn();
    return "ok";
  } catch (e) {
    return (e as Error).name;
  }
}

const GRID = `
struct O { @builtin(position) p: vec4f, @location(0) @interpolate(flat) i: u32 };
@vertex fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> O {
  let pos = array(vec2f(0.0, 0.25), vec2f(-0.25, -0.25), vec2f(0.25, -0.25));
  let center = vec2f(-2.0 / 3.0 + f32(i % 3u) * (2.0 / 3.0), 2.0 / 3.0 - f32(i / 3u) * (2.0 / 3.0));
  return O(vec4f(pos[v] * 0.9 + center, 0.0, 1.0), i);
}
@fragment fn fs(@location(0) @interpolate(flat) i: u32) -> @location(0) vec4f {
  return vec4f(f32(i + 1u) / 9.0, 0.0, 0.0, 1.0);
}`;

describe("GPUCommandEncoder and passes", () => {
  test("classes, labels and illegal constructors", async () => {
    const d = await device();
    const encoder = d.createCommandEncoder({ label: "enc" });
    expect(encoder instanceof CommandEncoderClass).toBe(true);
    expect(encoder.label).toBe("enc");
    const pass = encoder.beginComputePass({ label: "cp" });
    expect(pass instanceof ComputePassClass).toBe(true);
    expect(pass.label).toBe("cp");
    pass.end();
    const commands = encoder.finish({ label: "cb" });
    expect(commands instanceof CommandBufferClass).toBe(true);
    expect(commands.label).toBe("cb");
    expect(() => new (RenderPassClass as unknown as new () => unknown)()).toThrow("Illegal constructor");
    expect(Object.prototype.toString.call(encoder)).toBe("[object GPUCommandEncoder]");
  });

  test("[EnforceRange] arguments are TypeErrors outside their range", async () => {
    const d = await device();
    const buffer = d.createBuffer({ size: 16, usage: GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST });
    const encoder = d.createCommandEncoder();
    // EnforceRange.spec.ts "GPUSize64 as a direct argument".
    expect(thrown(() => encoder.copyBufferToBuffer(buffer, -4, buffer, 0, 4))).toBe("TypeError");
    expect(thrown(() => encoder.clearBuffer(buffer, NaN))).toBe("TypeError");
    const pass = encoder.beginComputePass();
    expect(thrown(() => pass.dispatchWorkgroups(2 ** 32))).toBe("TypeError");
    expect(thrown(() => pass.dispatchWorkgroups(1.9))).toBe("ok");
    pass.end();
  });

  test("a call on a locked encoder fails finish() with a validation error", async () => {
    const d = await device();
    const encoder = d.createCommandEncoder();
    const pass = encoder.beginComputePass();
    encoder.insertDebugMarker("while locked");
    pass.end();
    d.pushErrorScope("validation");
    const commands = encoder.finish();
    const error = await d.popErrorScope();
    expect(error?.message ?? "").toMatch(/insertDebugMarker was called while a pass was open/);
    d.pushErrorScope("validation");
    d.queue.submit([commands]);
    expect((await d.popErrorScope()) !== null).toBe(true);
  });

  test("draw, instanced, indexed and indirect draws render the same grid", async () => {
    const d = await device();
    const module = d.createShaderModule({ code: GRID });
    const pipeline = d.createRenderPipeline({
      layout: "auto",
      vertex: { module },
      fragment: { module, targets: [{ format: "rgba8unorm" }] },
    });
    const render = async (record: (pass: GPURenderPassEncoder | GPURenderBundleEncoder) => void, bundle: boolean) => {
      const texture = d.createTexture({
        size: [48, 48],
        format: "rgba8unorm",
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      });
      const encoder = d.createCommandEncoder();
      const pass = encoder.beginRenderPass({
        colorAttachments: [{ view: texture.createView(), clearValue: [0, 0, 0, 1], loadOp: "clear", storeOp: "store" }],
      });
      if (bundle) {
        const be = d.createRenderBundleEncoder({ colorFormats: ["rgba8unorm"] });
        expect(be instanceof RenderBundleEncoderClass).toBe(true);
        record(be);
        const b = be.finish({ label: "grid" });
        expect(b instanceof RenderBundleClass).toBe(true);
        pass.executeBundles([b]);
      } else {
        record(pass);
      }
      pass.end();
      d.pushErrorScope("validation");
      d.queue.submit([encoder.finish()]);
      expect(await d.popErrorScope()).toBe(null);
      return readTexture(d, texture, 48, 48);
    };
    const direct = await render((p) => {
      p.setPipeline(pipeline);
      p.draw(3, 9);
    }, false);
    // Row 1, column 2 is instance 5: red 6/9.
    expect(direct[(24 * 48 + 40) * 4]).toBe(Math.round((6 / 9) * 255));
    const indirect = d.createBuffer({ size: 16, usage: GPUBufferUsage.INDIRECT | GPUBufferUsage.COPY_DST });
    d.queue.writeBuffer(indirect, 0, new Uint32Array([3, 9, 0, 0]));
    const index = d.createBuffer({ size: 8, usage: GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST });
    d.queue.writeBuffer(index, 0, new Uint16Array([0, 1, 2, 0]));
    const variants: [string, (p: GPURenderPassEncoder | GPURenderBundleEncoder) => void, boolean][] = [
      ["indirect", (p) => (p.setPipeline(pipeline), p.drawIndirect(indirect, 0)), false],
      ["indexed", (p) => (p.setPipeline(pipeline), p.setIndexBuffer(index, "uint16"), p.drawIndexed(3, 9)), false],
      ["bundle", (p) => (p.setPipeline(pipeline), p.draw(3, 9)), true],
    ];
    for (const [, record, bundle] of variants) {
      expect((await render(record, bundle)).join()).toBe(direct.join());
    }
  });

  test("setBindGroup takes a sequence or a Uint32Array range of dynamic offsets", async () => {
    const d = await device();
    const uniforms = d.createBuffer({ size: 512, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    d.queue.writeBuffer(uniforms, 256, new Uint32Array([42]));
    d.queue.writeBuffer(uniforms, 0, new Uint32Array([7]));
    const out = d.createBuffer({ size: 4, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC });
    const layout = d.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: 4, buffer: { type: "uniform", hasDynamicOffset: true } },
        { binding: 1, visibility: 4, buffer: { type: "storage" } },
      ],
    });
    const pipeline = d.createComputePipeline({
      layout: d.createPipelineLayout({ bindGroupLayouts: [layout] }),
      compute: {
        module: d.createShaderModule({
          code: `@group(0) @binding(0) var<uniform> u: u32;
                 @group(0) @binding(1) var<storage, read_write> o: u32;
                 @compute @workgroup_size(1) fn main() { o = u; }`,
        }),
      },
    });
    const group = d.createBindGroup({
      layout,
      entries: [
        { binding: 0, resource: { buffer: uniforms, size: 4 } },
        { binding: 1, resource: { buffer: out } },
      ],
    });
    const run = async (set: (p: GPUComputePassEncoder) => void) => {
      const encoder = d.createCommandEncoder();
      const pass = encoder.beginComputePass();
      pass.setPipeline(pipeline);
      set(pass);
      pass.dispatchWorkgroups(1);
      pass.end();
      d.queue.submit([encoder.finish()]);
      return new Uint32Array((await readBack(d, out, 4)).buffer)[0];
    };
    expect(await run((p) => p.setBindGroup(0, group, [256]))).toBe(42);
    expect(await run((p) => p.setBindGroup(0, group, new Uint32Array([0, 256, 0]), 1, 1))).toBe(42);
    expect(await run((p) => p.setBindGroup(0, group, new Uint32Array([0, 256]), 0, 1))).toBe(7);
    const encoder = d.createCommandEncoder();
    const pass = encoder.beginComputePass();
    expect(thrown(() => pass.setBindGroup(0, group, new Uint32Array([0]), 1, 1))).toBe("RangeError");
    pass.end();
  });
});

describe("GPUQueue writes", () => {
  async function written(write: (d: GPUDevice, b: GPUBuffer) => void, size: number): Promise<Uint8Array> {
    const d = await device();
    const b = d.createBuffer({ size, usage: GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST });
    write(d, b);
    await d.queue.onSubmittedWorkDone();
    return readBack(d, b, size);
  }

  test("a typed-array view writes its own range, dataOffset and size count elements", async () => {
    // ArrayBufferBounds.spec.ts.
    const view = new Float32Array([10, 20, 30, 40]).subarray(1, 3);
    const a = await written((d, b) => d.queue.writeBuffer(b, 0, view), 8);
    expect(Array.from(new Float32Array(a.buffer))).toEqual([20, 30]);
    const c = await written((d, b) => d.queue.writeBuffer(b, 0, new Uint32Array([1, 2, 3, 4]), 1, 2), 8);
    expect(Array.from(new Uint32Array(c.buffer))).toEqual([2, 3]);
    const e = await written((d, b) => d.queue.writeBuffer(b, 0, new Uint8Array([5, 6, 7, 8]).buffer), 4);
    expect(Array.from(e)).toEqual([5, 6, 7, 8]);
  });

  test("look-alike buffer sources and ranges outside the data throw", async () => {
    const d = await device();
    const dst = d.createBuffer({ size: 256, usage: GPUBufferUsage.COPY_DST });
    for (const spoofed of [
      { buffer: new ArrayBuffer(4), byteOffset: 0, byteLength: 1 << 24, BYTES_PER_ELEMENT: 1 },
      { buffer: new ArrayBuffer(4), byteOffset: 1 << 24, byteLength: 4, BYTES_PER_ELEMENT: 1 },
      { buffer: new ArrayBuffer(16), byteOffset: -8, byteLength: 8, BYTES_PER_ELEMENT: 1 },
    ]) {
      expect(thrown(() => d.queue.writeBuffer(dst, 0, spoofed as unknown as ArrayBuffer))).toBe("TypeError");
    }
    expect(thrown(() => d.queue.writeBuffer(dst, 0, new Uint32Array(4), 5))).toBe("OperationError");
    expect(thrown(() => d.queue.writeBuffer(dst, 0, new Uint32Array(4), 1, 4))).toBe("OperationError");
    expect(thrown(() => d.queue.writeBuffer(dst, 0, new Uint8Array(6)))).toBe("OperationError");
    expect(thrown(() => d.queue.writeBuffer(dst, -4, new Uint8Array(4)))).toBe("TypeError");
  });

  test("writeTexture respects a view's byteOffset (#328)", async () => {
    const d = await device();
    const big = new Uint8Array(512);
    for (let i = 0; i < 256; i += 4) big.set([255, 0, 0, 255], i);
    for (let i = 256; i < 512; i += 4) big.set([0, 255, 0, 255], i);
    const texture = d.createTexture({
      size: [64, 1],
      format: "rgba8unorm",
      usage: GPUTextureUsage.COPY_SRC | GPUTextureUsage.COPY_DST,
    });
    d.queue.writeTexture({ texture }, new Uint8Array(big.buffer, 256, 256), { bytesPerRow: 256 }, { width: 64, height: 1 });
    expect((await readTexture(d, texture, 64, 1)).slice(0, 4)).toEqual([0, 255, 0, 255]);
    // A Uint32Array: dataLayout.offset still counts bytes.
    d.queue.writeTexture({ texture }, new Uint32Array(big.buffer), { offset: 0, bytesPerRow: 256 }, [64, 1]);
    expect((await readTexture(d, texture, 64, 1)).slice(0, 4)).toEqual([255, 0, 0, 255]);
  });
});

describe("createImageBitmap and copyExternalImageToTexture", () => {
  test("decodes an ArrayBuffer, a view and a sub-view (ImageData.spec.ts); close() zeroes the size, twice", async () => {
    const png = await asset("opaque-gradient.png");
    for (const source of [png.slice().buffer as ArrayBuffer, png]) {
      const bitmap = await createImageBitmap(source);
      expect([bitmap.width, bitmap.height]).toEqual([32, 32]);
    }
    const padded = new Uint8Array(png.length + 256).fill(0xff);
    padded.set(png, 128);
    const sub = await createImageBitmap(new Uint8Array(padded.buffer, 128, png.length));
    expect([sub.width, sub.height]).toEqual([32, 32]);
    expect(typeof sub.close).toBe("function");
    sub.close();
    sub.close();
    expect([sub.width, sub.height]).toEqual([0, 0]);
  });

  test("rejects look-alike sources with a TypeError and bad bytes with an InvalidStateError", async () => {
    const spoofed = { buffer: new ArrayBuffer(4), byteOffset: 0, byteLength: 1 << 24, BYTES_PER_ELEMENT: 1 };
    await expect(createImageBitmap(spoofed as unknown as ArrayBuffer)).rejects.toMatchObject({ name: "TypeError" });
    await expect(createImageBitmap(new Uint8Array([1, 2, 3, 4]))).rejects.toMatchObject({ name: "InvalidStateError" });
  });

  test("the crop overload, resize and an ImageData source", async () => {
    const png = await asset("opaque-gradient.png");
    const cropped = await createImageBitmap(png, 8, 4, 16, 24);
    expect([cropped.width, cropped.height]).toEqual([16, 24]);
    const resized = await createImageBitmap(png, { resizeWidth: 64 });
    expect([resized.width, resized.height]).toEqual([64, 64]);
    const data = new Uint8ClampedArray([255, 0, 0, 128, 0, 0, 255, 255]);
    const fromData = await createImageBitmap({ data, width: 2, height: 1 });
    expect([fromData.width, fromData.height]).toEqual([2, 1]);
  });

  test("premultiplied and straight alpha land as the destination asks", async () => {
    const d = await device();
    const png = await asset("alpha-gradient.png");
    const copy = async (premultiplyAlpha: "none" | "premultiply", premultipliedAlpha: boolean, flipY = false) => {
      const bitmap = await createImageBitmap(png, { premultiplyAlpha });
      const texture = d.createTexture({
        size: [32, 32],
        format: "rgba8unorm",
        usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.COPY_SRC | GPUTextureUsage.RENDER_ATTACHMENT,
      });
      d.queue.copyExternalImageToTexture({ source: bitmap, flipY }, { texture, premultipliedAlpha }, [32, 32]);
      return readTexture(d, texture, 32, 32);
    };
    const straight = await copy("none", false);
    const premultiplied = await copy("none", true);
    // ImageBitmap.h convertAlpha: (v * a + 127) / 255.
    for (let i = 0; i < straight.length; i += 4) {
      const a = straight[i + 3];
      expect(premultiplied[i]).toBe(Math.floor((straight[i] * a + 127) / 255));
    }
    expect((await copy("premultiply", true)).join()).toBe(premultiplied.join());
    const flipped = await copy("none", false, true);
    expect(flipped.slice(0, 128).join()).toBe(straight.slice(31 * 128).join());
  });

  test("an origin outside the bitmap is an OperationError, a closed bitmap an InvalidStateError", async () => {
    const d = await device();
    const bitmap = await createImageBitmap(await asset("opaque-gradient.png"));
    const texture = d.createTexture({
      size: [16, 32],
      format: "rgba8unorm",
      usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
    });
    expect(thrown(() => d.queue.copyExternalImageToTexture({ source: bitmap, origin: [17, 0] }, { texture }, [16, 32]))).toBe(
      "OperationError",
    );
    expect(thrown(() => d.queue.copyExternalImageToTexture({ source: bitmap, origin: [8, 0] }, { texture }, [16, 32]))).toBe(
      "ok",
    );
    bitmap.close();
    expect(thrown(() => d.queue.copyExternalImageToTexture({ source: bitmap }, { texture }, [16, 32]))).toBe(
      "InvalidStateError",
    );
  });
});

describe("GPUExternalTexture", () => {
  test("imports a GPUTexture; other sources say ERR_WEBGPU_UNSUPPORTED", async () => {
    const d = await device();
    const texture = d.createTexture({ size: [4, 4], format: "rgba8unorm", usage: GPUTextureUsage.TEXTURE_BINDING });
    // react-native-webgpu's global augmentations type these: no casts.
    const external = d.importExternalTexture({ source: texture, rotation: 90, mirrored: true });
    expect(external instanceof ExternalTextureClass).toBe(true);
    external.destroy();
    external.destroy();
    const frame = { pointer: 1n, width: 4, height: 4, pixelFormat: "bgra8" as const, release() {} };
    expect(() => d.importExternalTexture({ source: frame })).toThrow(
      "ERR_WEBGPU_UNSUPPORTED",
    );
  });
});
