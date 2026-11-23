// The README's example and the guide's samples (samples/), kept honest:
// `nsh check` type-checks every file in samples/; `cargo test --test
// readme` checks that the README's code block is samples/readme.app.tsx
// byte for byte; this test mounts it headless (the canvas renders
// offscreen: no native view here).

import { expect, open, test } from "nsh/test";

test("the README's example mounts and draws without an error", async () => {
  const app = await open("./samples/readme.app.tsx");
  // Let the ref arrive and the async draw finish (an error would fail the test).
  for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 10));
  expect(Array.isArray(app.texts())).toBe(true);
});
