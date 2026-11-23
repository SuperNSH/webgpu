// The canvas example on a real platform: `nsh test --target macos` (or
// `ios`) opens the app in the platform's host, where `webgpu.canvas` is the
// native view, and waits for the Triangle and 70 Cube frames. The app logs
// each canvas's texture as it was presented, captured from the surface
// (`PNG <name>-surface <base64>`, capture.ts), which the evidence keeps:
// app.screenshot() of the window gives a blank picture on a Mac without
// the Screen Recording permission. Headless, the canvases render offscreen
// and the same statuses show (`surface none`, `-offscreen`).

import { expect, open, test } from "nsh/test";

test("the Triangle and the Cube draw into their canvases", async () => {
  const app = await open("./App.tsx");
  await app.waitUntil(
    () => app.texts().some((t) => /^cube \(.*\): ([7-9]\d|\d{3,}) frames$/.test(t) || t.includes("failed")),
    { timeout: 30_000 },
  );
  const status = app.texts().find((t) => t.startsWith("WebGPU canvas:")) ?? "";
  console.log(`${status} | ${app.texts().find((t) => t.startsWith("cube")) ?? ""}`);
  expect(status).toMatch(/^WebGPU canvas: bgra8unorm triangle \d+x\d+ cube \d+x\d+ surface (attached|none)$/);
});
