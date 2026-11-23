// Area (a) resources, through the spec classes in the test runner: the
// reference's Buffer.spec, Texture.spec, Shaders.spec, Constants.spec,
// EnforceRange.spec, ErrorScope.spec, NullSlots.spec (bind group layouts)
// and GPU.spec (features, limits, info), plus the spec's mapping rules.
// Pixel comparisons against the reference's snapshots are cargo tests
// (tests/resources): this runner has no PNG decoder.

import { describe, expect, test } from "nsh/test";
import {
  GPUAdapterInfo,
  GPUBindGroup,
  GPUBindGroupLayout,
  GPUBufferUsage,
  GPUCompilationMessage,
  GPUComputePipeline,
  GPUDevice,
  GPUMapMode,
  GPUPipelineError,
  GPUPipelineLayout,
  GPUQuerySet,
  GPUSampler,
  GPUShaderStage,
  GPUSupportedFeatures,
  GPUSupportedLimits,
  GPUTextureUsage,
  GPUTextureView,
  GPUValidationError,
} from "nsh-webgpu";

const triangleVertWGSL = `@vertex
fn main(
  @builtin(vertex_index) VertexIndex : u32
) -> @builtin(position) vec4f {
  var pos = array<vec2f, 3>(
    vec2(0.0, 0.5),
    vec2(-0.5, -0.5),
    vec2(0.5, -0.5)
  );

  return vec4f(pos[VertexIndex], 0.0, 1.0);
}`;

const redFragWGSL = `@fragment
fn main() -> @location(0) vec4f {
  return vec4(1.0, 0.0, 0.0, 1.0);
}`;

async function device(descriptor?: GPUDeviceDescriptor): Promise<GPUDevice> {
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("No adapter");
  return (await adapter.requestDevice(descriptor)) as unknown as GPUDevice;
}

/** The validation error `f` makes, its message, or null. */
async function validationError(d: GPUDevice, f: () => unknown): Promise<string | null> {
  d.pushErrorScope("validation");
  f();
  const error = await d.popErrorScope();
  return error ? error.message : null;
}

/** What `f` throws: "ok", or the error's name. */
function attempt(f: () => unknown): string {
  try {
    f();
    return "ok";
  } catch (e) {
    return (e as Error).name;
  }
}

describe("Buffer.spec", () => {
  test("labels and metadata", async () => {
    const d = await device();
    const b1 = d.createBuffer({ size: 1440, usage: GPUBufferUsage.VERTEX, mappedAtCreation: true });
    const b2 = d.createBuffer({ size: 1440, usage: GPUBufferUsage.VERTEX, mappedAtCreation: true, label: "verticesBuffer" });
    expect([b1.label, b2.label]).toEqual(["", "verticesBuffer"]);
    expect([b1.label, b1.size, b1.usage, b1.mapState]).toEqual(["", 1440, 32, "mapped"]);
    const b3 = d.createBuffer({ size: 1440, usage: GPUBufferUsage.VERTEX, mappedAtCreation: false });
    expect([b3.label, b3.size, b3.usage, b3.mapState]).toEqual(["", 1440, 32, "unmapped"]);
  });

  test("upload data (1) and (2)", async () => {
    const d = await device();
    const cube = new Float32Array(360).map((_, i) => i / 7);
    for (const ranged of [false, true]) {
      const b = d.createBuffer({ size: cube.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_SRC, mappedAtCreation: true });
      new Float32Array(ranged ? b.getMappedRange(0, cube.byteLength) : b.getMappedRange()).set(cube);
      b.unmap();
      const read = d.createBuffer({ size: cube.byteLength, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
      const encoder = d.createCommandEncoder();
      encoder.copyBufferToBuffer(b, 0, read, 0, cube.byteLength);
      d.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      expect(Array.from(new Float32Array(read.getMappedRange()))).toEqual(Array.from(cube));
    }
  });

  test("read/write buffer (1), (2), (3)", async () => {
    const d = await device();
    const data = new Uint32Array([1, 2, 3, 4]);
    const a = d.createBuffer({ size: data.byteLength, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
    d.queue.writeBuffer(a, 0, data);
    await a.mapAsync(GPUMapMode.READ);
    expect(Array.from(new Uint32Array(a.getMappedRange()))).toEqual([1, 2, 3, 4]);
    a.unmap();
    const b = d.createBuffer({ size: data.byteLength, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
    d.queue.writeBuffer(b, 0, data.buffer, 0, data.byteLength);
    await b.mapAsync(GPUMapMode.READ, 0, data.byteLength);
    expect(Array.from(new Uint32Array(b.getMappedRange(0, data.byteLength)))).toEqual([1, 2, 3, 4]);
    b.unmap();
    await b.mapAsync(GPUMapMode.READ, 0, data.byteLength);
    expect(Array.from(new Float32Array(b.getMappedRange(0, data.byteLength)))).toEqual(
      Array.from(new Float32Array(data.buffer)),
    );
  });

  test("writes into a buffer (2) and (3): MAP_WRITE round trip", async () => {
    const d = await device();
    const size = 16;
    const source = d.createBuffer({ size, usage: GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST });
    const read = d.createBuffer({ size, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    d.queue.writeBuffer(source, 0, new Float32Array([1, 2, 3, 4]));
    let encoder = d.createCommandEncoder();
    encoder.copyBufferToBuffer(source, 0, read, 0, size);
    d.queue.submit([encoder.finish()]);
    await read.mapAsync(GPUMapMode.READ);
    expect(Array.from(new Float32Array(read.getMappedRange()))).toEqual([1, 2, 3, 4]);
    read.unmap();
    const write = d.createBuffer({ size, usage: GPUBufferUsage.COPY_SRC | GPUBufferUsage.MAP_WRITE });
    await write.mapAsync(GPUMapMode.WRITE);
    new Float32Array(write.getMappedRange()).set([5, 6, 7, 8]);
    write.unmap();
    encoder = d.createCommandEncoder();
    encoder.copyBufferToBuffer(write, 0, source, 0, size);
    encoder.copyBufferToBuffer(source, 0, read, 0, size);
    d.queue.submit([encoder.finish()]);
    await read.mapAsync(GPUMapMode.READ);
    expect(Array.from(new Float32Array(read.getMappedRange()))).toEqual([5, 6, 7, 8]);
  });

  test("writeBuffer respects TypedArray byteOffset (issue #328)", async () => {
    const d = await device();
    const backing = new ArrayBuffer(16);
    new Uint32Array(backing).set([111, 222, 333, 444]);
    const b = d.createBuffer({ size: 8, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
    d.queue.writeBuffer(b, 0, new Uint32Array(backing, 8, 2));
    await b.mapAsync(GPUMapMode.READ);
    expect(Array.from(new Uint32Array(b.getMappedRange()))).toEqual([333, 444]);
  });

  test("writes an image into a buffer and reads it back", async () => {
    const d = await device();
    const data = new Uint8Array(256 * 256 * 4);
    data.fill(255);
    let i = 0;
    for (let x = 0; x < 256 * 4; x++) {
      for (let y = 0; y < 256 * 4; y++) {
        data[i++] = (x * y) % 255;
      }
    }
    const b = d.createBuffer({ size: data.byteLength, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
    d.queue.writeBuffer(b, 0, data.buffer, 0, data.byteLength);
    await b.mapAsync(GPUMapMode.READ, 0, data.byteLength);
    const read = new Uint8Array(b.getMappedRange(0, data.byteLength));
    // snapshots/buffer.png is these bytes (cargo test compares the PNG).
    let same = read.length === data.length;
    for (let j = 0; same && j < read.length; j++) same = read[j] === data[j];
    expect(same).toBe(true);
  });
});

describe("GPUBuffer mapping", () => {
  test("mapAsync rejects invalid maps with an OperationError and a validation error", async () => {
    const d = await device();
    const b = d.createBuffer({ size: 64, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
    for (const [mode, offset, size] of [
      [GPUMapMode.WRITE, 0, 64],
      [GPUMapMode.READ, 4, 8],
      [GPUMapMode.READ, 0, 6],
      [GPUMapMode.READ, 32, 64],
    ]) {
      d.pushErrorScope("validation");
      await expect(b.mapAsync(mode, offset, size)).rejects.toMatchObject({ name: "OperationError" });
      expect((await d.popErrorScope()) instanceof GPUValidationError).toBe(true);
      expect(b.mapState).toBe("unmapped");
    }
    const first = b.mapAsync(GPUMapMode.READ);
    // While pending: OperationError, no validation error.
    await expect(b.mapAsync(GPUMapMode.READ)).rejects.toMatchObject({ name: "OperationError" });
    await first;
    await expect(b.mapAsync(GPUMapMode.READ)).rejects.toMatchObject({ name: "OperationError" });
    expect(b.mapState).toBe("mapped");
  });

  test("getMappedRange: ranges, alignment, overlap, detaching", async () => {
    const d = await device();
    const b = d.createBuffer({ size: 64, usage: GPUBufferUsage.MAP_WRITE | GPUBufferUsage.COPY_SRC });
    expect(() => b.getMappedRange()).toThrow("not mapped");
    await b.mapAsync(GPUMapMode.WRITE, 16, 32);
    const first = b.getMappedRange(16, 8);
    const second = b.getMappedRange(24);
    expect([first.byteLength, second.byteLength]).toEqual([8, 24]);
    for (const [offset, size] of [
      [0, 8],
      [40, 16],
      [20, 4],
      [32, 6],
      [16, 8],
    ]) {
      expect(attempt(() => b.getMappedRange(offset, size))).toBe("OperationError");
    }
    b.unmap();
    expect([first.byteLength, second.byteLength]).toEqual([0, 0]);
    expect(b.mapState).toBe("unmapped");
  });

  test("destroy() while mapped or mapping", async () => {
    const d = await device();
    const mapped = d.createBuffer({ size: 16, usage: GPUBufferUsage.COPY_SRC, mappedAtCreation: true });
    const range = mapped.getMappedRange();
    mapped.destroy();
    expect(mapped.mapState).toBe("unmapped");
    expect(range.byteLength).toBe(0);
    const pending = d.createBuffer({ size: 16, usage: GPUBufferUsage.MAP_READ });
    const mapping = pending.mapAsync(GPUMapMode.READ);
    pending.destroy();
    await expect(mapping).rejects.toMatchObject({ name: "AbortError" });
    expect(pending.mapState).toBe("unmapped");
    d.pushErrorScope("validation");
    await expect(pending.mapAsync(GPUMapMode.READ)).rejects.toMatchObject({ name: "OperationError" });
    expect(await d.popErrorScope()).not.toBeNull();
  });

  test("mappedAtCreation needs a multiple of 4", async () => {
    const d = await device();
    expect(() => d.createBuffer({ size: 6, usage: GPUBufferUsage.VERTEX, mappedAtCreation: true })).toThrow(RangeError);
  });
});

describe("Texture.spec", () => {
  test("usage, descriptor round trip", async () => {
    const d = await device();
    expect(GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING).toBe(20);
    const t = d.createTexture({
      size: [512, 512],
      format: "rgba8unorm",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      label: "texture",
    });
    expect([t.label, t.width, t.height, t.depthOrArrayLayers, t.mipLevelCount, t.sampleCount, t.dimension, t.format, t.usage]).toEqual(
      ["texture", 512, 512, 1, 1, 1, "2d", "rgba8unorm", 20],
    );
  });

  test("write to texture and read it back", async () => {
    const d = await device();
    const [width, height] = [800, 600];
    const bytesPerRow = Math.ceil((width * 4) / 256) * 256;
    const texture = d.createTexture({
      size: [width, height],
      format: "rgba8unorm",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
    });
    const pipeline = d.createRenderPipeline({
      layout: "auto",
      vertex: { module: d.createShaderModule({ code: triangleVertWGSL }), entryPoint: "main" },
      fragment: {
        module: d.createShaderModule({ code: redFragWGSL }),
        entryPoint: "main",
        targets: [{ format: "rgba8unorm" }],
      },
      primitive: { topology: "triangle-list" },
    });
    const output = d.createBuffer({ size: bytesPerRow * height, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    const encoder = d.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [{ view: texture.createView(), clearValue: [0, 0, 0, 1], loadOp: "clear", storeOp: "store" }],
    });
    pass.setPipeline(pipeline);
    pass.draw(3);
    pass.end();
    encoder.copyTextureToBuffer({ texture }, { buffer: output, bytesPerRow }, [width, height]);
    d.queue.submit([encoder.finish()]);
    expect([texture.label, texture.width, texture.height, texture.depthOrArrayLayers, texture.mipLevelCount, texture.sampleCount, texture.dimension, texture.format, texture.usage]).toEqual(
      ["", 800, 600, 1, 1, 1, "2d", "rgba8unorm", 17],
    );
    await output.mapAsync(GPUMapMode.READ);
    const pixels = new Uint8Array(output.getMappedRange());
    const at = (x: number, y: number) => Array.from(pixels.subarray(y * bytesPerRow + x * 4, y * bytesPerRow + x * 4 + 4));
    expect(at(400, 300)).toEqual([255, 0, 0, 255]);
    expect(at(10, 10)).toEqual([0, 0, 0, 255]);
  });

  test("every dimension and view", async () => {
    const d = await device();
    const t = d.createTexture({ size: { width: 8, height: 8, depthOrArrayLayers: 12 }, format: "rgba8unorm", usage: GPUTextureUsage.TEXTURE_BINDING, mipLevelCount: 4 });
    expect([t.depthOrArrayLayers, t.mipLevelCount]).toEqual([12, 4]);
    const error = await validationError(d, () => {
      for (const descriptor of [
        undefined,
        { dimension: "2d-array" },
        { dimension: "cube", baseArrayLayer: 6, arrayLayerCount: 6 },
        { dimension: "cube-array", arrayLayerCount: 12 },
        { baseMipLevel: 1, mipLevelCount: 2, dimension: "2d-array", label: "v" },
        { usage: GPUTextureUsage.TEXTURE_BINDING, aspect: "all" },
      ] as GPUTextureViewDescriptor[]) {
        expect(t.createView(descriptor) instanceof GPUTextureView).toBe(true);
      }
      d.createTexture({ size: [16], dimension: "1d", format: "r8unorm", usage: GPUTextureUsage.TEXTURE_BINDING }).createView({ dimension: "1d" });
      d.createTexture({ size: [4, 4, 4], dimension: "3d", format: "rgba8unorm", usage: GPUTextureUsage.TEXTURE_BINDING }).createView({ dimension: "3d" });
    });
    expect(error).toBeNull();
    expect(t.createView({ label: "named" }).label).toBe("named");
    expect(await validationError(d, () => t.createView({ baseMipLevel: 4 }))).not.toBeNull();
    expect(attempt(() => d.createTexture({ size: [4, 4], format: "r64uint" as GPUTextureFormat, usage: 1 }))).toBe("TypeError");
    expect(attempt(() => d.createTexture({ size: [], format: "rgba8unorm", usage: 1 }))).toBe("TypeError");
  });
});

describe("EnforceRange.spec", () => {
  test("TypeError for integers outside their range", async () => {
    const d = await device();
    expect([
      attempt(() => d.createBuffer({ size: -1, usage: GPUBufferUsage.COPY_DST })),
      attempt(() => d.createBuffer({ size: NaN, usage: GPUBufferUsage.COPY_DST })),
      attempt(() => d.createTexture({ size: [4, 4], format: "rgba8unorm", usage: GPUTextureUsage.COPY_DST, mipLevelCount: Infinity })),
      attempt(() => d.createQuerySet({ type: "occlusion", count: 2 ** 32 })),
      attempt(() => d.createBuffer({ size: 4, usage: 2 ** 32 })),
    ]).toEqual(["TypeError", "TypeError", "TypeError", "TypeError", "TypeError"]);
  });

  test("truncates fractional integers toward zero", async () => {
    const d = await device();
    const buffer = d.createBuffer({ size: 16.9, usage: GPUBufferUsage.COPY_DST });
    const texture = d.createTexture({ size: [4, 4], format: "rgba8unorm", usage: GPUTextureUsage.COPY_DST, mipLevelCount: 2.5 });
    expect([buffer.size, texture.mipLevelCount]).toEqual([16, 2]);
  });
});

describe("Shaders.spec", () => {
  test("labels, no messages, pipeline label", async () => {
    const d = await device();
    expect(d.createShaderModule({ code: triangleVertWGSL, label: "triangleVertWGSL" }).label).toBe("triangleVertWGSL");
    const frag = d.createShaderModule({ code: redFragWGSL, label: "redFragWGSL" });
    expect(frag.label).toBe("redFragWGSL");
    expect((await frag.getCompilationInfo()).messages.length).toBe(0);
    const pipeline = d.createRenderPipeline({
      layout: "auto",
      vertex: { module: d.createShaderModule({ code: triangleVertWGSL }) },
      fragment: { module: frag, targets: [{ format: navigator.gpu.getPreferredCanvasFormat() }] },
      primitive: { topology: "triangle-list" },
      label: "trianglePipeline",
    });
    expect(pipeline.label).toBe("trianglePipeline");
  });

  test("error messages carry the spec's positions", async () => {
    const d = await device();
    d.pushErrorScope("validation");
    const module = d.createShaderModule({ code: "// é\nfn f() -> f32 { return nope; }" });
    expect(await d.popErrorScope()).not.toBeNull();
    const [message] = (await module.getCompilationInfo()).messages;
    expect(message instanceof GPUCompilationMessage).toBe(true);
    expect([message.type, message.lineNum, message.linePos, message.offset, message.length]).toEqual([
      "error",
      2,
      24,
      28,
      4,
    ]);
    expect(message.message).toContain("nope");
  });
});

describe("Constants.spec", () => {
  test("flag values and instanceof", async () => {
    expect([GPUBufferUsage.MAP_READ, GPUBufferUsage.MAP_WRITE, GPUBufferUsage.COPY_SRC, GPUBufferUsage.COPY_DST, GPUBufferUsage.INDEX, GPUBufferUsage.VERTEX, GPUBufferUsage.UNIFORM, GPUBufferUsage.STORAGE, GPUBufferUsage.INDIRECT, GPUBufferUsage.QUERY_RESOLVE]).toEqual(
      [1, 2, 4, 8, 16, 32, 64, 128, 256, 512],
    );
    expect([GPUTextureUsage.COPY_SRC, GPUTextureUsage.COPY_DST, GPUTextureUsage.TEXTURE_BINDING, GPUTextureUsage.STORAGE_BINDING, GPUTextureUsage.RENDER_ATTACHMENT]).toEqual([1, 2, 4, 8, 16]);
    expect([GPUMapMode.READ, GPUMapMode.WRITE]).toEqual([1, 2]);
    expect([GPUShaderStage.VERTEX, GPUShaderStage.FRAGMENT, GPUShaderStage.COMPUTE]).toEqual([1, 2, 4]);
    const d = await device();
    expect(d.createSampler() instanceof GPUSampler).toBe(true);
    expect(d.createSampler() instanceof globalThis.GPUSampler).toBe(true);
    expect(d.createQuerySet({ type: "occlusion", count: 1 }) instanceof GPUQuerySet).toBe(true);
  });

  test("__brand names the class; nativePointer is ERR_WEBGPU_UNSUPPORTED", async () => {
    const d = await device();
    const texture = d.createTexture({ size: [1, 1], format: "rgba8unorm", usage: GPUTextureUsage.COPY_SRC });
    expect([
      d.createBuffer({ size: 4, usage: GPUBufferUsage.COPY_SRC }).__brand,
      texture.__brand,
      texture.createView().__brand,
      d.createSampler().__brand,
      d.limits.__brand,
      d.adapterInfo.__brand,
    ]).toEqual(["GPUBuffer", "GPUTexture", "GPUTextureView", "GPUSampler", "GPUSupportedLimits", "GPUAdapterInfo"]);
    expect(() => texture.nativePointer).toThrow("ERR_WEBGPU_UNSUPPORTED");
  });
});

describe("Samplers and bind groups", () => {
  test("ErrorScope.spec: an invalid maxAnisotropy is a validation error", async () => {
    const d = await device();
    d.pushErrorScope("validation");
    d.createSampler({ maxAnisotropy: 0 });
    const error = await d.popErrorScope();
    expect(error instanceof GPUValidationError).toBe(true);
    expect(error!.message.length).toBeGreaterThan(0);
    expect(await validationError(d, () => d.createSampler({ magFilter: "linear", minFilter: "linear", mipmapFilter: "linear", maxAnisotropy: 16, compare: "less", label: "s" }))).toBeNull();
    expect(attempt(() => d.createSampler({ lodMinClamp: NaN }))).toBe("TypeError");
  });

  test("Device.spec: createBindGroup accepts a raw GPUBuffer and a GPUBufferBinding", async () => {
    const d = await device();
    const layout = d.createBindGroupLayout({
      entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: "uniform" } }],
    });
    expect(layout instanceof GPUBindGroupLayout).toBe(true);
    const buffer = d.createBuffer({ size: 256, usage: GPUBufferUsage.UNIFORM });
    const error = await validationError(d, () => {
      expect(d.createBindGroup({ layout, entries: [{ binding: 0, resource: buffer }] }) instanceof GPUBindGroup).toBe(true);
      d.createBindGroup({ layout, entries: [{ binding: 0, resource: { buffer, offset: 0, size: 64 } }] });
    });
    expect(error).toBeNull();
    expect(attempt(() => d.createBindGroup({ layout, entries: [{ binding: 0, resource: {} as GPUBufferBinding }] }))).toBe("TypeError");
  });

  test("explicit layouts of every kind, a texture as a resource", async () => {
    const d = await device();
    const layout = d.createBindGroupLayout({
      label: "all",
      entries: [
        { binding: 0, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: {} },
        { binding: 2, visibility: GPUShaderStage.COMPUTE, storageTexture: { format: "rgba8unorm" } },
        { binding: 3, visibility: GPUShaderStage.COMPUTE, buffer: { type: "read-only-storage" } },
      ],
    });
    const texture = d.createTexture({ size: [4, 4], format: "rgba8unorm", usage: GPUTextureUsage.TEXTURE_BINDING });
    const storage = d.createTexture({ size: [4, 4], format: "rgba8unorm", usage: GPUTextureUsage.STORAGE_BINDING });
    const error = await validationError(d, () =>
      d.createBindGroup({
        layout,
        entries: [
          { binding: 0, resource: d.createSampler() },
          { binding: 1, resource: texture },
          { binding: 2, resource: storage.createView() },
          { binding: 3, resource: { buffer: d.createBuffer({ size: 16, usage: GPUBufferUsage.STORAGE }) } },
        ],
      }),
    );
    expect(error).toBeNull();
    expect(
      await validationError(d, () =>
        d.createBindGroupLayout({ entries: [{ binding: 0, visibility: GPUShaderStage.COMPUTE, buffer: {}, sampler: {} }] }),
      ),
    ).not.toBeNull();
  });

  test("NullSlots.spec: a null bind group layout keeps the later indices", async () => {
    const d = await device();
    const module = d.createShaderModule({
      code: `@group(1) @binding(0) var<storage, read_write> out: u32;
             @compute @workgroup_size(1) fn main() { out = 42u; }`,
    });
    const layout = d.createBindGroupLayout({
      entries: [{ binding: 0, visibility: GPUShaderStage.COMPUTE, buffer: { type: "storage" } }],
    });
    const error = await validationError(d, () => {
      const pipelineLayout = d.createPipelineLayout({ bindGroupLayouts: [null, layout] });
      expect(pipelineLayout instanceof GPUPipelineLayout).toBe(true);
      const pipeline = d.createComputePipeline({ layout: pipelineLayout, compute: { module } });
      expect(pipeline instanceof GPUComputePipeline).toBe(true);
      // An undefined and a hole are null too.
      // eslint-disable-next-line no-sparse-arrays
      d.createPipelineLayout({ bindGroupLayouts: [undefined, layout] as unknown as GPUBindGroupLayout[] });
    });
    expect(error).toBeNull();
  });
});

describe("Compute pipelines", () => {
  test("getBindGroupLayout and the async variant", async () => {
    const d = await device();
    const module = d.createShaderModule({
      code: `@group(0) @binding(0) var<storage, read_write> out: array<f32>;
             @compute @workgroup_size(1) fn main() { out[0] = 1.0; }`,
    });
    const pipeline = await d.createComputePipelineAsync({ layout: "auto", compute: { module, entryPoint: "main" }, label: "c" });
    expect(pipeline instanceof GPUComputePipeline).toBe(true);
    expect(pipeline.label).toBe("c");
    expect(pipeline.getBindGroupLayout(0) instanceof GPUBindGroupLayout).toBe(true);
    const failed = await d.createComputePipelineAsync({ layout: "auto", compute: { module, entryPoint: "missing" } }).then(
      () => null,
      (e: unknown) => e as GPUPipelineError,
    );
    expect(failed instanceof GPUPipelineError).toBe(true);
    expect(failed!.reason).toBe("validation");
    await expect(d.popErrorScope()).rejects.toMatchObject({ name: "OperationError" });
    expect(await validationError(d, () => d.createComputePipeline({ layout: "auto", compute: { module, entryPoint: "missing" } }))).not.toBeNull();
  });
});

describe("Query sets", () => {
  test("type, count, destroy, timestamp without the feature", async () => {
    const d = await device();
    const set = d.createQuerySet({ type: "occlusion", count: 4, label: "q" });
    expect([set.type, set.count, set.label]).toEqual(["occlusion", 4, "q"]);
    set.destroy();
    set.destroy();
    expect(await validationError(d, () => d.createQuerySet({ type: "timestamp", count: 2 }))).toContain("timestamp-query");
    expect(await validationError(d, () => d.createQuerySet({ type: "occlusion", count: 4097 }))).not.toBeNull();
  });
});

describe("GPU.spec: features, limits, info", () => {
  test("features are a set of spec names and every one can be required", async () => {
    const adapter = (await navigator.gpu.requestAdapter())!;
    const features = Array.from(adapter.features);
    expect(adapter.features instanceof GPUSupportedFeatures).toBe(true);
    for (const name of ["depth-clip-control", "rg11b10ufloat-renderable", "texture-compression-etc2", "core-features-and-limits"]) {
      expect(features).toContain(name);
    }
    expect(features).not.toContain("");
    expect(adapter.features.size).toBe(features.length);
    const d = await adapter.requestDevice({ requiredFeatures: features as GPUFeatureName[] });
    expect(Array.from(d.features).sort()).toEqual([...features].sort());
    if (adapter.features.has("timestamp-query")) {
      expect(d.features.has("timestamp-query")).toBe(true);
    }
  });

  test("limits: every getter, bounds, JSON.stringify", async () => {
    const adapter = (await navigator.gpu.requestAdapter())!;
    const limits = adapter.limits;
    expect(limits instanceof GPUSupportedLimits).toBe(true);
    expect(limits.maxBindGroupsPlusVertexBuffers).toBeGreaterThan(limits.maxBindGroups);
    expect(Math.log2(limits.minStorageBufferOffsetAlignment) % 1).toBe(0);
    expect(limits.maxTextureDimension3D).toBeGreaterThanOrEqual(256);
    expect(limits.maxStorageBuffersInFragmentStage).toBe(limits.maxStorageBuffersPerShaderStage);
    const json = JSON.parse(JSON.stringify(limits)) as Record<string, number>;
    expect(json.maxBindGroups).toBeGreaterThan(0);
    expect(json.maxBufferSize).toBeGreaterThan(0);
    expect(Object.keys(json).length).toBe(36);
    const d = await adapter.requestDevice({ requiredLimits: { maxBufferSize: 4 * 1024 * 1024 } });
    expect(d.limits.maxBufferSize).toBe(256 * 1024 * 1024);
  });

  test("info: fields, fallback, JSON, the device's equals the adapter's", async () => {
    const adapter = (await navigator.gpu.requestAdapter())!;
    const info = adapter.info;
    expect(info instanceof GPUAdapterInfo).toBe(true);
    for (const value of [info.vendor, info.architecture, info.device, info.description]) {
      expect(typeof value).toBe("string");
    }
    expect(info.isFallbackAdapter).toBe(/swiftshader|llvmpipe|lavapipe/i.test(`${info.architecture} ${info.description}`));
    const d = await adapter.requestDevice();
    expect(JSON.parse(JSON.stringify(d.adapterInfo))).toEqual(JSON.parse(JSON.stringify(info)));
    expect(JSON.parse(JSON.stringify(info)).description).toBe(info.description);
  });
});
