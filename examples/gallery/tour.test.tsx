// The on-screen tour for the evidence (not a check): with
// `nsh test --target ios tour.test.tsx` it opens every Home entry in the
// simulator on the real clock, waits for the demo to draw, logs
// `TOUR <entry>` and holds the page for a while; a script outside watches
// the log and takes `xcrun simctl io <udid> screenshot` at each line
// (docs/evidence/gallery/). On a phone target `open` refuses a viewport
// width (ERR_NSH_TEST_UNSUPPORTED); headless it takes one, and the test
// returns at once, so plain `nsh test` does not tour.

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
  "tour: every demo on screen, for the screenshots (phone targets only)",
  async () => {
    try {
      const headless = await open("./App.tsx", { width: 390 });
      headless.dispose();
      console.log("tour skipped: not on a phone target");
      return;
    } catch {
      // A phone: tour.
    }
    const app = await open("./App.tsx", { network: "live" });
    const shoot = async (name: string) => {
      console.log(`TOUR ${name}`);
      await app.wait(HOLD_MS);
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
