// Phase 3 (integration): what joins the areas. `installWebGPU()` stands down
// on the web, `<Canvas>` takes the web branch there, and the offscreen
// readback module is registered with the native module.

import { describe, expect, open, test } from "nsh/test";
import "nsh-webgpu";
import { WgpuOffscreenCanvas } from "nsh-webgpu/native";

describe("<Canvas> and installWebGPU() by platform", () => {
  test("on the web: the browser's navigator.gpu stays, the canvas is a web canvas view", async () => {
    const app = await open("./integration.app.tsx", { platform: "web" });
    expect(app.texts()).toContain("os: web");
    expect(app.texts()).toContain("navigator.gpu: none");
    // The polyfill's ids count from 1, and the surface is the id (BigInt);
    // the ref arrives once the canvas is laid out.
    await app.waitFor("text:context id: 1");
    expect(app.texts()).toContain("surface: 1");
    await app.waitFor("id:rnwgpu-canvas-1");
    expect(app.count("id:rnwgpu-canvas-1")).toBe(1);
  });

  test("off the web: navigator.gpu is nsh-webgpu's and the canvas is the native view", async () => {
    const app = await open("./integration.app.tsx", { platform: "macos" });
    expect(app.texts()).toContain("os: macos");
    expect(app.texts()).toContain("navigator.gpu: nsh-webgpu");
    await app.waitFor("text:surface: 0");
    expect(app.count("id:rnwgpu-canvas-1")).toBe(0);
  });
});

test("the offscreen readback module is registered", () => {
  expect(WgpuOffscreenCanvas.size(999_999)).toBe(null);
  expect(() => WgpuOffscreenCanvas.takePixels(999_999)).toThrow("no readback 999999");
});

test("the reference's extensions wgpu cannot back throw ERR_WEBGPU_UNSUPPORTED", async () => {
  // The global augmentations type the extensions on the spec's interfaces.
  const device = await (await navigator.gpu.requestAdapter())!.requestDevice({
    dawnToggles: { enabledToggles: ["skip_validation"], disabledToggles: [] },
    implicitDeviceSynchronization: true,
  });
  const queue = device.queue;
  const code = (f: () => unknown) => {
    try {
      f();
      return "no error";
    } catch (error) {
      return (error as { code?: string }).code;
    }
  };
  expect(code(() => device.importSharedTextureMemory({ handle: 1n }))).toBe("ERR_WEBGPU_UNSUPPORTED");
  expect(code(() => device.importSharedFence({ type: "sync-fd", handle: 1n }))).toBe("ERR_WEBGPU_UNSUPPORTED");
  expect(code(() => (device as unknown as { forceLossForTesting(): void }).forceLossForTesting())).toBe(
    "ERR_WEBGPU_UNSUPPORTED",
  );
  const texture = device.createTexture({ size: [4, 4], format: "rgba8unorm", usage: GPUTextureUsage.RENDER_ATTACHMENT });
  const frame = { pointer: 1n, width: 4, height: 4, pixelFormat: "bgra8" as const, release() {} };
  expect(code(() => queue.copyExternalImageToTexture({ source: frame }, { texture }, [4, 4]))).toBe("ERR_WEBGPU_UNSUPPORTED");
  await expect(
    queue.drawElementImageToTexture({ source: 1 }, { texture: device.createTexture({ size: [1, 1], format: "rgba8unorm", usage: GPUTextureUsage.COPY_DST }) }),
  ).rejects.toMatchObject({ code: "ERR_WEBGPU_UNSUPPORTED" });
});
