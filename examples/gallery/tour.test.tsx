// The on-screen tour for the evidence (not a check): with
// `nsh test --target <platform> tour.test.tsx` it opens every Home entry in
// the platform's host on the real clock and waits for the demo to draw.
// - macos, linux, windows: it saves a picture of the window of each page to
//   `tour/<entry>.png` (make the `tour` directory first).
// - ios, android: it logs `TOUR <entry>` and holds the page for a while; a
//   script outside watches the log and takes the device's screenshot at each
//   line (docs/evidence/gallery/). On a phone `open` refuses a viewport
//   width (ERR_NSH_TEST_UNSUPPORTED).
// Headless there is no picture to take and the test returns at once, so plain
// `nsh test` does not tour.

import { expect, open, test } from "nsh/test";
import { decodePng } from "./src/harness/png";

/** Every demo of the Home list, then the Diagnostics list's ported screens. */
const DEMOS = [
  "HelloTriangle",
  "HelloTriangleMSAA",
  "ImageBlur",
  "ComputeToys",
  "Cube",
  "CanvasAPI",
  "Cubemap",
  "InstancedCube",
  "TexturedCube",
  "FractalCube",
  "Particles",
  "RenderBundles",
  "OcclusionQuery",
  "ReversedZ",
  "ComputeBoids",
  "MNISTInference",
  "ShadowMapping",
  "SamplerParameters",
  "DeferedRendering",
  "ABuffer",
  "Wireframe",
  "Resize",
  "GradientTiles",
  "StorageBufferVertices",
  "SixteenBitTextures",
];
const SKIPPED = ["Tests", "ThreeJS"];
const DIAGNOSTICS = ["SurfaceChurn", "RenderAfterUnmount", "ContextEdgeCases"];

const HOLD_MS = 3000;

test(
  "tour: every demo on screen, for the screenshots (real platforms only)",
  async () => {
    let desktop = true;
    try {
      const probe = await open("./App.tsx", { width: 390 });
      try {
        await probe.screenshot();
      } catch {
        console.log("tour skipped: headless");
        return;
      } finally {
        probe.dispose();
      }
    } catch {
      desktop = false;
    }
    const app = await open("./App.tsx", desktop ? { network: "live", width: 480, height: 640 } : { network: "live" });
    const shoot = async (name: string) => {
      console.log(`TOUR ${name}`);
      if (desktop) await app.screenshot(`tour/${name}.png`);
      else await app.wait(HOLD_MS);
    };
    // A demo that draws slowly (a software GPU) takes a while to let go of the page.
    const home = async () => {
      await app.back();
      await app.waitUntil(() => app.texts().includes("WebGPU Gallery"), { timeout: 60_000 });
    };
    await app.wait(1500);
    await shoot("Home");
    for (const screen of DEMOS) {
      await app.press(`id:open-${screen}`);
      if (screen === "CanvasAPI") {
        await app.wait(500);
        await app.press("id:check-surface");
      }
      if (screen === "MNISTInference") {
        await app.wait(4000);
        await app.drag("id:mnist-pad", { x: 0, y: 200 });
      }
      try {
        await app.waitUntil(
          () => app.texts().some((t) => / frame=| failed/.test(t) && t.startsWith(`probe ${screen}:`)),
          { timeout: 30_000 },
        );
      } catch {
        // Shot as it is (a download that did not come, say).
      }
      await app.wait(1000);
      if (screen === "OcclusionQuery") {
        // Its camera looks past every cube now and then: the labels say which it sees.
        await app.waitUntil(() => app.texts().some((t) => /^(🟥|🟨|🟩|🟧|🟦|🟪)+$/u.test(t)), { timeout: 30_000 });
      }
      await shoot(screen);
      if (desktop && screen === "SixteenBitTextures") {
        // The same colors in an 8-bit and a 16-bit float canvas: the window shows them alike.
        const picture = decodePng(await app.screenshot());
        const at = (x: number) => {
          const i = (Math.round(picture.height * 0.4) * picture.width + Math.round(picture.width * x)) * 4;
          return [...picture.rgba.slice(i, i + 3)];
        };
        const [eight, sixteen] = [at(0.25), at(0.75)];
        console.log(`16-bit canvas: ${sixteen}, 8-bit: ${eight}`);
        for (let c = 0; c < 3; c++) expect(Math.abs(eight[c] - sixteen[c])).toBeLessThan(12);
      }
      await home();
    }
    for (const screen of SKIPPED) {
      await app.press(`id:open-${screen}`);
      await shoot(`skip-${screen}`);
      await home();
    }
    await app.press("id:open-Diagnostics");
    await shoot("Diagnostics");
    for (const screen of DIAGNOSTICS) {
      await app.press(`id:open-${screen}`);
      if (screen === "SurfaceChurn") await app.press("id:churn-toggle");
      if (screen === "ContextEdgeCases") {
        await app.press("id:edge-unconfigured");
        await app.press("id:edge-zero");
        await app.press("id:edge-unconfigure");
      }
      await app.wait(3000);
      await shoot(`diag-${screen}`);
      // A device the page destroys is lost as "destroyed"; any other loss is the GPU's.
      expect(app.texts().filter((t) => t.startsWith("[device lost] unknown"))).toEqual([]);
      await app.back();
    }
    await home();
    await app.press("id:open-ImportExternalTexture");
    await app.wait(3000);
    // Not on Vulkan or GL: wgpu 29 has external textures on Metal and DX12 only.
    await shoot("ImportExternalTexture");
    expect(app.texts().filter((t) => / failed/.test(t) && !t.includes("ERR_WEBGPU_UNSUPPORTED"))).toEqual([]);
    console.log("TOUR done");
  },
  20 * 60_000,
);
