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

import { open, test } from "nsh/test";

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
      await shoot(screen);
      await app.back();
    }
    for (const screen of SKIPPED) {
      await app.press(`id:open-${screen}`);
      await shoot(`skip-${screen}`);
      await app.back();
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
      await app.back();
    }
    // Last: Import External Texture fails its first frame (a crate bug), and
    // leaving the page then aborts the host (KNOWN_ISSUES.md), so the tour
    // ends on it.
    await app.back();
    await app.back();
    await app.press("id:open-ImportExternalTexture");
    await app.wait(3000);
    await shoot("ImportExternalTexture");
    console.log("TOUR done");
  },
  20 * 60_000,
);
