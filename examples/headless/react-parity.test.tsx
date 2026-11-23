// react-native-webgpu's React API on NSH (area: react-parity): the hooks,
// the provider, the offscreen canvas and the extensions, through the UI of
// react-parity.app.tsx on a manual clock with no network, and the stubs'
// errors directly.

import { describe, expect, open, test } from "nsh/test";
import {
  adoptTexture,
  createNativeBuffer,
  createNativeBufferFromPointer,
  createTestVideoFrame,
  createVideoPlayer,
  getPreferredHighBitDepthCanvasFormat,
  GPUOffscreenCanvas,
  importDevice,
  installWebGPU,
  WebGPUModule,
  type NativeBuffer,
  warnIfNotHardwareAccelerated,
} from "nsh-webgpu";
// Not a package export (the reference reaches it through its RNWebGPU
// global): the nv12 variant of createTestVideoFrame, read from the source.
import { testVideoFrame } from "../../tsx/video";

/** createTestVideoFrame's frame as nsh-webgpu makes it: in memory, its bytes in plane(i). */
type InMemoryFrame = NativeBuffer & { plane(index?: number): Uint8Array };

const scenario = (name: string) => open("./react-parity.app.tsx", { clock: {}, host: { params: { scenario: name } } });
const GPU_WAIT = { timeout: 20_000 };

describe("hooks", () => {
  test("useDevice resolves an adapter and a device; useCanvasRef and useSurface follow the canvas", async () => {
    const app = await scenario("device");
    await app.waitFor("text:device: hooked", GPU_WAIT);
    expect(app.texts()).toContain("adapter: hardware");
    expect(app.texts()).toContain("canvas ref: number");
    expect(app.texts()).toContain("surface: 7x5");
  });

  test("warnIfNotHardwareAccelerated warns for a fallback adapter only", () => {
    const warned: string[] = [];
    const original = console.warn;
    console.warn = (...args: unknown[]) => void warned.push(args.join(" "));
    try {
      warnIfNotHardwareAccelerated({ info: { isFallbackAdapter: false } } as GPUAdapter);
      expect(warned).toEqual([]);
      warnIfNotHardwareAccelerated({ info: { isFallbackAdapter: true } } as GPUAdapter);
    } finally {
      console.warn = original;
    }
    expect(warned).toEqual([
      "GPUAdapter is not hardware accelerated. This is common on Android emulators. Rendering will be slow. Some features may be unavailable.",
    ]);
  });
});

describe("GPUDeviceProvider", () => {
  test("renders its children once the device is ready and shares one device; useMainDevice throws outside", async () => {
    const app = await scenario("provider");
    expect(app.texts()).toContain("outside: No DeviceContext found.");
    await app.waitFor("text:main device: provided, adapter: yes", GPU_WAIT);
    expect(app.texts().filter((t) => t === "main device: provided, adapter: yes").length).toBe(2);
  });
});

describe("GPUOffscreenCanvas", () => {
  test("renders the reference's triangle and reads it back with getImageData", async () => {
    const app = await scenario("offscreen");
    await app.waitUntil(() => app.texts().some((t) => t.startsWith("offscreen: ") && t !== "offscreen: rendering"), GPU_WAIT);
    const shown = app.texts().find((t) => t.startsWith("offscreen: ")) ?? "";
    expect(shown).toMatch(/^offscreen: bgra8unorm 50x50 bytes=10000 center=0,0,255,255 corner=0,0,0,0 red=\d+$/);
    // Half of a 25x25 box: about 312 pixels.
    const red = Number(/red=(\d+)/.exec(shown)?.[1]);
    expect(red > 250 && red < 380).toBe(true);
  });

  // The frame reaches the canvas by a render pass: its texture has no
  // COPY_DST, as the reference's Offscreen.ts makes it.
  test("a frame of createTestVideoFrame written into it reads back byte for byte", async () => {
    const app = await scenario("offscreen");
    await app.waitUntil(() => app.texts().some((t) => t.startsWith("frame: ") && t !== "frame: rendering"), GPU_WAIT);
    // The last pixel: x=49, y=2 → B 220 (stripe on), G 255, R 255, A 255.
    expect(app.texts()).toContain("frame: bgra8unorm same=true last=220,255,255,255");
  });

  test("getContext gives the one context for webgpu and null otherwise; it throws before configure", () => {
    const canvas = new GPUOffscreenCanvas(64, 64);
    const context = canvas.getContext("webgpu")!;
    expect(canvas.getContext("webgpu")).toBe(context);
    expect(canvas.getContext("2d" as "webgpu")).toBe(null);
    expect(context.canvas).toBe(canvas);
    expect(() => context.getCurrentTexture()).toThrow("Texture is not configured");
    expect(() => context.getConfiguration()).toThrow("Method not implemented.");
    expect(() => canvas.getImageData()).toThrow("Device is not configured.");
  });
});

describe("useNativeBufferSize", () => {
  test("reports the first size at once and settles later ones after 150 ms", async () => {
    const app = await scenario("buffersize");
    expect(app.texts()).toContain("size: none");
    await app.press("id:report-100");
    expect(app.texts()).toContain("size: 100x50");
    await app.press("id:report-200");
    await app.clock!.runFor(100);
    await app.press("id:report-300");
    await app.clock!.runFor(149);
    expect(app.texts()).toContain("size: 100x50");
    await app.clock!.runFor(1);
    expect(app.texts()).toContain("size: 300x90");
  });

  test("NativeBufferView throws ERR_WEBGPU_UNSUPPORTED", async () => {
    const app = await scenario("buffersize");
    expect(app.texts()).toContain("view: ERR_WEBGPU_UNSUPPORTED");
  });
});

describe("extensions", () => {
  test("createTestVideoFrame makes the reference's bgra8 pattern", () => {
    // The reference's export: (width, height) => NativeBuffer. nsh-webgpu's
    // frame lives in memory, its bytes in plane(i).
    expect(createTestVideoFrame.length).toBe(2);
    const frame = createTestVideoFrame(64, 64) as InMemoryFrame;
    expect([frame.handle, frame.width, frame.height, frame.pixelFormat, frame.acquireFenceFd]).toEqual([
      0n,
      64,
      64,
      "bgra8",
      null,
    ]);
    const bytes = frame.plane();
    expect(bytes.length).toBe(64 * 64 * 4);
    const px = (x: number, y: number) => Array.from(bytes.slice((y * 64 + x) * 4, (y * 64 + x) * 4 + 4));
    expect(px(0, 0)).toEqual([30, 0, 0, 255]);
    expect(px(63, 63)).toEqual([220, 255, 255, 255]);
    expect(px(32, 0)).toEqual([220, 0, 129, 255]);
    frame.release();
    expect(() => frame.plane()).toThrow("released");
  });

  test("RNWebGPU.createTestVideoFrame's nv12 frame (internal here): red left, green right", () => {
    const frame = testVideoFrame(8, 4, "nv12");
    expect(frame.pixelFormat).toBe("nv12");
    expect(Array.from(frame.plane(0).slice(0, 8))).toEqual([63, 63, 63, 63, 173, 173, 173, 173]);
    expect(Array.from(frame.plane(1).slice(0, 8))).toEqual([102, 240, 102, 240, 42, 26, 42, 26]);
    expect(frame.plane(1).length).toBe(4 * 2 * 2);
    expect(() => createTestVideoFrame(0, 4)).toThrow(RangeError);
    // The public export takes no pixel format, as the reference's.
    expect((createTestVideoFrame as (w: number, h: number, f: string) => NativeBuffer)(8, 4, "nv12").pixelFormat).toBe(
      "bgra8",
    );
  });

  test("the Dawn and platform-buffer extensions throw ERR_WEBGPU_UNSUPPORTED", () => {
    const unsupported = { code: "ERR_WEBGPU_UNSUPPORTED" };
    const thrown = (f: () => unknown) => {
      try {
        f();
      } catch (error) {
        return error;
      }
      return null;
    };
    expect(thrown(() => importDevice(1n))).toMatchObject(unsupported);
    expect(thrown(() => adoptTexture(1n))).toMatchObject(unsupported);
    expect(thrown(() => createVideoPlayer("file:///v.mp4"))).toMatchObject(unsupported);
    expect(thrown(() => createNativeBufferFromPointer(1n))).toMatchObject(unsupported);
    expect(thrown(() => createNativeBuffer(4, 4))).toMatchObject(unsupported);
    expect(() => importDevice(1n)).toThrow("importDevice is unavailable");
  });

  test("getPreferredHighBitDepthCanvasFormat, WebGPUModule.install and installWebGPU", () => {
    expect(getPreferredHighBitDepthCanvasFormat()).toBe("rgba16float");
    expect(WebGPUModule.install()).toBe(true);
    installWebGPU();
    expect(typeof navigator.gpu.requestAdapter).toBe("function");
  });
});
