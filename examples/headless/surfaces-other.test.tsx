// Area surfaces-other in TSX: the web canvas (tsx/surface/web.tsx), headless.
// Off the web it stands down (no browser GPU, no canvas, no context); with
// `platform: "web"` its view is a view named after the context
// (`rnwgpu-canvas-<id>`, where the browser's `<canvas>` goes). The browser
// itself is examples/canvas-web, run with `nsh run --target web`
// (docs/parity/surfaces-other.md "Web").

import { describe, expect, open, test } from "nsh/test";
import {
  allocateContextIdWeb,
  browserGPU,
  contextIdToId,
  getNativeSurfaceWeb,
  isWeb,
  makeWebGPUCanvasContextWeb,
  webCanvas,
} from "../../tsx/surface/web";

describe("the web canvas off the web", () => {
  test("has no browser GPU, canvas or context", () => {
    expect(isWeb()).toBe(false);
    expect(browserGPU()).toBe(null);
    expect(webCanvas(1)).toBe(null);
    expect(makeWebGPUCanvasContextWeb(1, 16, 16)).toBe(null);
  });

  test("names canvases as the reference does and counts context ids up", () => {
    expect(contextIdToId(7)).toBe("rnwgpu-canvas-7");
    const first = allocateContextIdWeb();
    expect(allocateContextIdWeb()).toBe(first + 1);
  });

  test("reports a canvas not in a page as 0 x 0 with inert DOM methods", () => {
    const surface = getNativeSurfaceWeb(3);
    expect(surface.surface).toBe(3n);
    expect([surface.width, surface.height, surface.clientWidth, surface.clientHeight]).toEqual([0, 0, 0, 0]);
    expect(surface.dispatchEvent("x")).toBe(false);
    surface.addEventListener("pointerdown", () => {});
  });
});

test("on the web the canvas view is a view named after its context, filling its parent", async () => {
  const app = await open("../canvas-web/App.tsx", { platform: "web" });
  await app.waitFor("id:rnwgpu-canvas-1");
  expect(app.count("id:rnwgpu-canvas-1")).toBe(1);
  const layout = app.layout("id:rnwgpu-canvas-1");
  expect(layout !== null && layout.width > 300 && layout.height > 300).toBe(true);
  // Headless has no DOM: the `<canvas>` never comes, so drawing never starts.
  expect(app.texts()).toContain("waiting for the canvas");
});
