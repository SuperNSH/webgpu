// The <Canvas> ref's lifecycle and useSurface() with the real <Canvas>,
// headless (canvas-lifecycle.app.tsx): as react-native-webgpu's, the ref
// is set once the canvas is mounted and laid out, gets null on unmount,
// and getContext() on an unmounted canvas throws.

import { expect, open, test } from "nsh/test";

test("the ref is set after the first layout, with a context of the layout's size", async () => {
  const app = await open("./canvas-lifecycle.app.tsx");
  await app.waitFor("text:refs: canvas");
  // Never during render: the view did not exist yet.
  expect(app.texts()).toContain("refs: canvas");
  expect(app.texts()).toContain("first context: 40x30");
});

test("useSurface() follows the real canvas: mount, resize, unmount", async () => {
  const app = await open("./canvas-lifecycle.app.tsx");
  await app.waitFor("text:surface: 40x20");
  expect(app.texts()).toContain("client: 40x20");
  await app.press("id:wider");
  await app.waitFor("text:surface: 56x20");
  await app.press("id:toggle");
  await app.waitFor("text:surface: none");
});

test("unmount calls the ref with null, and the old ref's getContext() throws", async () => {
  const app = await open("./canvas-lifecycle.app.tsx");
  await app.waitFor("text:refs: canvas");
  await app.press("id:toggle");
  await app.waitFor("text:refs: canvas,null");
  await app.press("id:stale");
  expect(app.texts()).toContain("stale getContext: [WebGPU] Cannot get context before mount");
  expect(app.texts()).toContain("object ref: null");
  // Mounted again: a new canvas, a new ref.
  await app.press("id:toggle");
  await app.waitFor("text:refs: canvas,null,canvas");
});

test("development warnings: HardwareBufferView below Android 10, a backing view changed on a mounted canvas", async () => {
  // The headless Android reports API level 0: below 10 and 11.
  const app = await open("./canvas-warnings.app.tsx", { platform: "android" });
  await app.waitFor("text:surface view");
  await app.press("id:surface-view");
  await app.waitUntil(() => app.texts().filter((t) => t.startsWith("warned: ")).length >= 2);
  expect(app.texts().filter((t) => t.startsWith("warned: "))).toEqual([
    "warned: android.backingView: HardwareBufferView needs Android 10, the canvas uses a TextureView.",
    "warned: Ignoring a change of android.backingView on a mounted canvas: the backing view is chosen when the canvas mounts. Remount the canvas (give it a new key) to change it.",
  ]);
});
