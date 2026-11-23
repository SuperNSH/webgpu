// The spec classes directly, in the test runner (no app): their shapes,
// errors and mapping, on this machine's GPU.

import { describe, expect, test } from "nsh/test";
import {
  GPUAdapter,
  GPUBindGroupLayout,
  GPUBuffer,
  GPUBufferUsage,
  GPUDevice,
  GPUMapMode,
  GPUPipelineError,
  GPUSupportedFeatures,
  GPUSupportedLimits,
  GPUTexture,
  GPUTextureUsage,
  GPUValidationError,
} from "nsh-webgpu";
import { Wgpu, WgpuDevice } from "nsh-webgpu/native";

async function device(): Promise<GPUDevice> {
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("No adapter");
  return (await adapter.requestDevice({ label: "test device" })) as unknown as GPUDevice;
}

describe("navigator.gpu", () => {
  test("is installed with the classes and constants as globals", async () => {
    expect(typeof navigator.gpu.requestAdapter).toBe("function");
    expect(navigator.gpu.getPreferredCanvasFormat()).toBe("bgra8unorm");
    expect(globalThis.GPUBufferUsage.MAP_READ).toBe(1);
    expect(GPUTextureUsage.RENDER_ATTACHMENT).toBe(16);
    const adapter = await navigator.gpu.requestAdapter();
    expect(adapter instanceof GPUAdapter).toBe(true);
    expect(adapter instanceof globalThis.GPUAdapter).toBe(true);
    expect(() => new (GPUBuffer as unknown as new () => unknown)()).toThrow("Illegal constructor");
  });

  test("the adapter has info, set-like features and limits", async () => {
    const adapter = (await navigator.gpu.requestAdapter())!;
    expect(adapter.info.description.length > 0).toBe(true);
    expect(adapter.features instanceof GPUSupportedFeatures).toBe(true);
    expect(typeof adapter.features.has("depth-clip-control")).toBe("boolean");
    expect([...adapter.features].every((f) => typeof f === "string")).toBe(true);
    expect(adapter.limits instanceof GPUSupportedLimits).toBe(true);
    expect(adapter.limits.maxTextureDimension2D >= 8192).toBe(true);
    expect(adapter.limits.maxBindGroups >= 4).toBe(true);
  });

  test("a second requestDevice on one adapter rejects", async () => {
    const adapter = (await navigator.gpu.requestAdapter())!;
    await adapter.requestDevice();
    await expect(adapter.requestDevice()).rejects.toMatchObject({ name: "OperationError" });
  });

  test("an unknown feature rejects with a TypeError", async () => {
    const adapter = (await navigator.gpu.requestAdapter())!;
    await expect(
      adapter.requestDevice({ requiredFeatures: ["no-such-feature" as GPUFeatureName] }),
    ).rejects.toMatchObject({ name: "TypeError" });
  });
});

describe("GPUDevice", () => {
  test("has its label, queue, limits and adapter info", async () => {
    const d = await device();
    expect(d.label).toBe("test device");
    expect(d instanceof EventTarget).toBe(true);
    expect(d.queue.label).toBe("");
    expect(d.limits.maxBindGroups >= 4).toBe(true);
    expect(d.adapterInfo.description.length > 0).toBe(true);
  });

  test("error scopes catch validation errors as GPUValidationError", async () => {
    const d = await device();
    d.pushErrorScope("validation");
    d.createBuffer({ size: 4, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.MAP_WRITE });
    const error = await d.popErrorScope();
    expect(error instanceof GPUValidationError).toBe(true);
    expect(error!.message.length > 0).toBe(true);
    d.pushErrorScope("out-of-memory");
    d.createBuffer({ size: 4, usage: GPUBufferUsage.COPY_DST });
    expect(await d.popErrorScope()).toBe(null);
    await expect(d.popErrorScope()).rejects.toMatchObject({ name: "OperationError" });
    expect(() => d.pushErrorScope("nope" as GPUErrorFilter)).toThrow(TypeError);
  });

  test("an error no scope catches is an uncapturederror event", async () => {
    const d = await device();
    const heard: string[] = [];
    d.addEventListener("uncapturederror", (event) => {
      heard.push((event as GPUUncapturedErrorEvent).error.constructor.name);
    });
    let handled = 0;
    d.onuncapturederror = () => handled++;
    d.createBuffer({ size: 4, usage: 0 });
    // The event is a task of its own, after this one.
    for (let i = 0; i < 50 && heard.length === 0; i++) await new Promise((r) => setTimeout(r, 0));
    expect(heard).toEqual(["GPUValidationError"]);
    expect(handled).toBe(1);
  });

  test("createRenderPipelineAsync rejects with a GPUPipelineError", async () => {
    const d = await device();
    const module = d.createShaderModule({ code: "@vertex fn main() -> @builtin(position) vec4f { return vec4f(); }" });
    const failed = await d
      .createRenderPipelineAsync({ layout: "auto", vertex: { module, entryPoint: "missing" } })
      .then(
        () => null,
        (error: unknown) => error as GPUPipelineError,
      );
    expect(failed instanceof GPUPipelineError).toBe(true);
    expect(failed instanceof DOMException).toBe(true);
    expect(failed!.reason).toBe("validation");
    // The scopes it opened are closed: none is left.
    await expect(d.popErrorScope()).rejects.toMatchObject({ name: "OperationError" });
  });

  test("create*PipelineAsync compile off the JS thread", async () => {
    const d = await device();
    // Long enough to compile that the JS thread gets turns meanwhile.
    const steps = Array.from({ length: 600 }, (_, i) => `x = sin(x * ${i + 1}.0) + cos(x + ${i}.0);`).join("\n");
    const code = `
      fn work(seed: f32) -> f32 { var x = seed; ${steps} return x; }
      @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f { return vec4f(work(f32(i)), 0.0, 0.0, 1.0); }
      @fragment fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f { return vec4f(work(p.x)); }
      @group(0) @binding(0) var<storage, read_write> out: array<f32>;
      @compute @workgroup_size(1) fn cs() { out[0] = work(out[0]); }`;
    const module = d.createShaderModule({ code });
    const turns = async (make: () => Promise<unknown>): Promise<number> => {
      let count = 0;
      let done = false;
      const tick = () => {
        if (done) return;
        count++;
        setTimeout(tick, 0);
      };
      setTimeout(tick, 0);
      await make();
      done = true;
      return count;
    };
    const render = await turns(() =>
      d.createRenderPipelineAsync({
        layout: "auto",
        vertex: { module, entryPoint: "vs" },
        fragment: { module, entryPoint: "fs", targets: [{ format: "rgba8unorm" }] },
      }),
    );
    const compute = await turns(() => d.createComputePipelineAsync({ layout: "auto", compute: { module, entryPoint: "cs" } }));
    console.log(`JS turns while compiling: render ${render}, compute ${compute}`);
    expect(render > 0).toBe(true);
    expect(compute > 0).toBe(true);
    // A pipeline made this way works as one made synchronously.
    const pipeline = await d.createComputePipelineAsync({ layout: "auto", compute: { module, entryPoint: "cs" } });
    expect(pipeline.getBindGroupLayout(0) instanceof GPUBindGroupLayout).toBe(true);
  });

  test("destroy() settles lost with reason destroyed", async () => {
    const d = await device();
    d.destroy();
    const info = await d.lost;
    expect(info.reason).toBe("destroyed");
  });

  test("destroy() releases the native device; its objects still behave as a lost device's", async () => {
    const handle = Symbol.for("nsh-webgpu.handle");
    const idOf = (d: GPUDevice) => (d as unknown as Record<symbol, number>)[handle];
    for (let i = 0; i < 40; i++) {
      const d = await device();
      expect(Wgpu.alive(idOf(d))).toBe(true);
      d.destroy();
      d.destroy();
      expect((await d.lost).reason).toBe("destroyed");
      // Its id and listeners went with destroy(), not with the collector.
      expect(Wgpu.alive(idOf(d))).toBe(false);
    }
    // A destroyed device's objects find it still: no exception, as in the spec.
    const d = await device();
    const buffer = d.createBuffer({ size: 16, usage: GPUBufferUsage.COPY_DST });
    d.destroy();
    expect(() => d.queue.writeBuffer(buffer, 0, new Uint8Array(16))).not.toThrow();
    expect(() => d.queue.submit([d.createCommandEncoder().finish()])).not.toThrow();
    d.pushErrorScope("validation");
    await d.popErrorScope();
  });

  test("a device the app drops is collected and released", async () => {
    const handle = Symbol.for("nsh-webgpu.handle");
    const before = WgpuDevice.liveCount();
    // Made in a function of their own, so no frame of this test holds one.
    const make = async (): Promise<number> => {
      const d = await device();
      // Listeners of its own do not keep it alive.
      d.onuncapturederror = () => {};
      d.addEventListener("uncapturederror", () => {});
      return (d as unknown as Record<symbol, number>)[handle];
    };
    const ids: number[] = [];
    for (let i = 0; i < 20; i++) ids.push(await make());
    expect(WgpuDevice.liveCount()).toBe(before + 20);
    const alive = () => ids.filter((id) => Wgpu.alive(id)).length;
    // QuickJS collects cycles when its heap grows: grow it until it did and
    // the finalizers ran.
    for (let round = 0; round < 200 && alive() > 0; round++) {
      const heap: object[] = [];
      for (let j = 0; j < 64; j++) heap.push(new ArrayBuffer(64 * 1024), { j });
      heap.length = 0;
      await new Promise((r) => setTimeout(r, 0));
    }
    expect(alive()).toBe(0);
    expect(WgpuDevice.liveCount() <= before).toBe(true);
  });
});

describe("GPUBuffer", () => {
  test("getters and labels", async () => {
    const d = await device();
    const buffer = d.createBuffer({ label: "b", size: 16, usage: GPUBufferUsage.COPY_SRC });
    expect(buffer.size).toBe(16);
    expect(buffer.usage).toBe(GPUBufferUsage.COPY_SRC);
    expect(buffer.mapState).toBe("unmapped");
    expect(buffer.label).toBe("b");
    buffer.label = "renamed";
    expect(buffer.label).toBe("renamed");
  });

  test("writes to a mapped range reach the buffer on unmap, and the range detaches", async () => {
    const d = await device();
    const source = d.createBuffer({ size: 16, usage: GPUBufferUsage.COPY_SRC, mappedAtCreation: true });
    expect(source.mapState).toBe("mapped");
    const range = source.getMappedRange();
    new Uint32Array(range).set([1, 2, 3, 4]);
    expect(() => source.getMappedRange(0, 4)).toThrow("overlaps");
    source.unmap();
    expect(range.byteLength).toBe(0);
    const target = d.createBuffer({ size: 16, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    const encoder = d.createCommandEncoder();
    encoder.copyBufferToBuffer(source, target);
    d.queue.submit([encoder.finish()]);
    const mapping = target.mapAsync(GPUMapMode.READ);
    expect(target.mapState).toBe("pending");
    await mapping;
    expect(target.mapState).toBe("mapped");
    expect(Array.from(new Uint32Array(target.getMappedRange()))).toEqual([1, 2, 3, 4]);
    target.unmap();
    expect(() => target.getMappedRange()).toThrow("not mapped");
  });

  test("queue.writeBuffer counts dataOffset and size in elements", async () => {
    const d = await device();
    const buffer = d.createBuffer({ size: 8, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    d.queue.writeBuffer(buffer, 0, new Uint32Array([7, 8, 9, 10]), 1, 2);
    await buffer.mapAsync(GPUMapMode.READ);
    expect(Array.from(new Uint32Array(buffer.getMappedRange()))).toEqual([8, 9]);
  });

  test("unmap() before the map finishes rejects mapAsync with an AbortError", async () => {
    const d = await device();
    const buffer = d.createBuffer({ size: 8, usage: GPUBufferUsage.MAP_READ });
    const mapping = buffer.mapAsync(GPUMapMode.READ);
    buffer.unmap();
    await expect(mapping).rejects.toMatchObject({ name: "AbortError" });
    expect(buffer.mapState).toBe("unmapped");
  });
});

describe("GPUTexture", () => {
  test("reports its descriptor", async () => {
    const d = await device();
    const texture = d.createTexture({
      size: { width: 8, height: 4 },
      format: "rgba8unorm",
      usage: GPUTextureUsage.TEXTURE_BINDING,
    });
    expect(texture instanceof GPUTexture).toBe(true);
    expect([texture.width, texture.height, texture.depthOrArrayLayers]).toEqual([8, 4, 1]);
    expect([texture.dimension, texture.format, texture.mipLevelCount, texture.sampleCount]).toEqual([
      "2d",
      "rgba8unorm",
      1,
      1,
    ]);
    texture.destroy();
  });

  test("an unsupported call says ERR_WEBGPU_UNSUPPORTED", async () => {
    const d = await device();
    const texture = d.createTexture({ size: [1, 1], format: "rgba8unorm", usage: GPUTextureUsage.TEXTURE_BINDING });
    expect(() => texture.nativePointer).toThrow("ERR_WEBGPU_UNSUPPORTED");
  });
});
