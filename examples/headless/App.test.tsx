import { expect, open, test } from "nsh/test";

test("the reference triangle renders offscreen through the TSX classes", async () => {
  const app = await open("./App.tsx");
  const done = () => app.texts().some((t) => t.startsWith("triangle:") && t !== "triangle: rendering");
  await app.waitUntil(done, { timeout: 20_000 });
  const shown = app.texts().find((t) => t.startsWith("triangle:"));
  expect(shown).toMatch(/^triangle: bgra8unorm center=255,0,0,255 corner=0,0,0,0 red=\d+$/);
  const red = Number(/red=(\d+)/.exec(shown ?? "")?.[1]);
  expect(red >= 480 && red <= 560).toBe(true);
});
