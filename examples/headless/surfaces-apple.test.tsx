// <Canvas> and GPUCanvasContext through the UI (area: surfaces-apple): the
// app surfaces-apple.app.tsx runs each check and shows `<name>: <value>`.
// Headless, no host registers `webgpu.canvas`, so the context renders
// offscreen, as before a view's surface attaches.

import { expect, open, test } from "nsh/test";

async function results(): Promise<Map<string, string>> {
  const app = await open("./surfaces-apple.app.tsx");
  const done = () => app.texts().some((t) => t.startsWith("animation frames:") || t.startsWith("failed:"));
  await app.waitUntil(done, { timeout: 20_000 });
  const map = new Map<string, string>();
  for (const text of app.texts()) {
    const i = text.indexOf(": ");
    if (i > 0) map.set(text.slice(0, i), text.slice(i + 2));
  }
  return map;
}

test("the canvas ref and the context's configuration", async () => {
  const r = await results();
  expect(r.get("failed")).toBe(undefined);
  expect(r.get("contextId")).toBe("number");
  expect(r.get("unsupported context")).toBe("Error: [WebGPU] Unsupported context: 2d");
  expect(r.get("native surface")).toBe("0");
  expect(r.get("instance")).toBe("true");
  // The canvas starts at the view's layout size in points, as the reference's.
  expect(r.get("canvas size")).toBe("64x64");
  expect(r.get("configuration before")).toBe("null");
  expect(r.get("texture before configure")).toMatch(/^InvalidStateError: getCurrentTexture\(\) called on a canvas context that is not configured/);
  expect(r.get("bad colorSpace")).toMatch(/^TypeError: .*PredefinedColorSpace/);
  expect(r.get("bad toneMapping")).toMatch(/^TypeError: .*GPUCanvasToneMappingMode/);
  expect(r.get("bad format")).toMatch(/^TypeError: /);
  expect(JSON.parse(r.get("configuration") ?? "null")).toEqual({
    device: true,
    format: "bgra8unorm",
    usage: 17,
    viewFormats: [],
    colorSpace: "srgb",
    toneMapping: { mode: "standard" },
    alphaMode: "premultiplied",
  });
  expect(r.get("shared configuration")).toBe("true");
  expect(r.get("configuration after unconfigure")).toBe("null");
  expect(r.get("texture after unconfigure")).toBe("InvalidStateError");
});

test("the current texture: one per frame, and the triangle drawn into it", async () => {
  const r = await results();
  expect(r.get("same texture in a frame")).toBe("true");
  expect(r.get("texture")).toBe("64x64 bgra8unorm 17 true");
  expect(r.get("triangle")).toBe("center=255,0,0,255 corner=0,0,0,0");
  expect(r.get("new texture after present")).toBe("true");
  expect(r.get("new texture after configure")).toBe("true");
  // canvas.width = 16.9 (a fractional pixel ratio) is 16 and resizes the drawing buffer at the next getCurrentTexture.
  expect(r.get("after resize")).toBe("true 16x64 of 16");
});

test("requestAnimationFrame drives a render loop", async () => {
  const r = await results();
  expect(r.get("animation frames")).toBe("3");
});
