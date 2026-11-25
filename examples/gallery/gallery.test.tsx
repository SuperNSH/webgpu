// The gallery, opened through its Home list on a manual clock, demo by
// demo: each demo's first frame (or the frame its probe names) is read back
// from the canvas texture and summarized by the probe (src/harness/probe.ts):
// the size, the GPU errors nobody caught, the number of distinct colors and
// the dominant ones. Each demo must have drawn without a GPU error, and
// its frame must look like the demo: the colors expected below, in the
// shares given. In snapshot mode, the demos the reference has a snapshot
// of are compared with it.
//
// Headless (`nsh test`), canvases render offscreen at the view's size;
// with `--target ios|macos` they render into the native view's surface.

import { type App, expect, open, type OpenOptions, test } from "nsh/test";

/** A dominant color expectation: `#rgba` (4 bits a channel), within `tolerance` steps, at least `min` percent. */
interface Color {
  rgba: string;
  min: number;
  tolerance?: number;
}

interface Expectation {
  /** Colors that must be among the five most common. */
  colors?: Color[];
  /** At least this many distinct quantized colors. */
  distinct?: number;
  /** What the app's requests get (the demos that download at run time); none: no network. */
  network?: OpenOptions["network"];
  /** The manual clock's start (`{}`: now). */
  clock?: OpenOptions["clock"];
  /** The demo fails with this message (a known crate bug), instead of drawing. */
  failure?: RegExp;
  /** The demo may fail with this message instead of drawing: what wgpu lacks on some backends. */
  unsupported?: RegExp;
  /** Steps on the demo's page before its frame is drawn (a button that draws). */
  before?: (app: App) => Promise<void>;
}

// The compute.toys shader and uniforms ComputeToys downloads, stood in for
// by a small toy of the same form (compute.toys' prelude: `screen`, `time`,
// `custom`): a UV gradient with a blue channel from a custom uniform.
const TOY_SHADER = `
@compute @workgroup_size(16, 16)
fn main_image(@builtin(global_invocation_id) id: vec3u) {
    let screen_size = textureDimensions(screen);
    if (id.x >= screen_size.x || id.y >= screen_size.y) { return; }
    let uv = vec2f(id.xy) / vec2f(screen_size);
    textureStore(screen, id.xy, vec4f(uv.x, uv.y, custom.blue, 1.0));
}
`;
const TOY_REPLIES = {
  wgsl: { status: 200, body: TOY_SHADER },
  json: {
    status: 200,
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ body: { uniforms: [{ name: "blue", value: 0.5 }] } }),
  },
};
// On the web the demo asks through the reference's CORS proxy.
const proxied = (url: string) => `GET https://corsproxy.io/?${encodeURIComponent(url)}`;
const TOY_NETWORK: OpenOptions["network"] = {
  "GET https://compute.toys/view/2349/wgsl": TOY_REPLIES.wgsl,
  "GET https://compute.toys/view/2349/json": TOY_REPLIES.json,
  [proxied("https://compute.toys/view/2349/wgsl")]: TOY_REPLIES.wgsl,
  [proxied("https://compute.toys/view/2349/json")]: TOY_REPLIES.json,
};

// A stub's body is a string (UTF-8 on the wire) or JSON, so the .npy files
// below are made of ASCII bytes only: the magic's first byte is 0x13 for
// 0x93 (the loader reads only the header length and the header), and every
// float is one whose four bytes are below 0x80.
const ascii = (b0: number, b1: number, b2: number, b3: number) =>
  new Float32Array(new Uint8Array([b0, b1, b2, b3]).buffer)[0];
const W = ascii(0x03, 0x03, 0x03, 0x3a); // ~0.0005
const BIAS = ascii(0x1a, 0x19, 0x19, 0x3e); // ~0.1495
const ONE = ascii(0x7f, 0x7f, 0x7f, 0x3f); // ~0.998

/** A .npy file of float32 values (version 1.0), as a string of ASCII bytes. */
function npy(shape: number[], values: Float32Array): string {
  let header = `{'descr': '<f4', 'fortran_order': False, 'shape': (${shape.join(", ")}${shape.length === 1 ? "," : ""}), }`;
  while ((10 + header.length + 1) % 64 !== 0) header += " ";
  header += "\n";
  const bytes = new Uint8Array(10 + header.length + values.byteLength);
  bytes.set([0x13, ...Array.from("NUMPY", (c) => c.charCodeAt(0)), 1, 0], 0);
  new DataView(bytes.buffer).setUint16(8, header.length, true);
  bytes.set(Array.from(header, (c) => c.charCodeAt(0)), 10);
  bytes.set(new Uint8Array(values.buffer), 10 + header.length);
  if (bytes.some((b) => b >= 0x80)) throw new Error("npy stub: a byte is not ASCII");
  return String.fromCharCode(...bytes);
}

// MNISTInference downloads 8 dense layers (TypeGPU's MNIST weights). The
// stand-in network has the same shape of files: 784 -> 10, then 10 -> 10
// seven times. Layer 0 sums the input into unit 0, with a bias that keeps a
// blank pad (every input -0.42) below 0 after the ReLU; the middle layers
// pass unit 0 on; the last one sends it to digit 7. So a blank pad guesses
// 0, and a stroke of about ten inked cells or more guesses 7.
const MNIST_NETWORK: OpenOptions["network"] = (() => {
  const stubs: Record<string, { status: number; body: string }> = {};
  const url = (name: string) => `GET https://docs.swmansion.com/TypeGPU/assets/mnist-weights/${name}`;
  for (let layer = 0; layer < 8; layer++) {
    const inputs = layer === 0 ? 784 : 10;
    const weights = new Float32Array(10 * inputs);
    const biases = new Float32Array(10);
    if (layer === 0) {
      for (let j = 0; j < inputs; j++) weights[j] = W;
      biases[0] = BIAS;
    } else if (layer < 7) {
      weights[0] = ONE;
    } else {
      weights[7 * inputs] = ONE;
    }
    stubs[url(`layer${layer}.weight.npy`)] = { status: 200, body: npy([10, inputs], weights) };
    stubs[url(`layer${layer}.bias.npy`)] = { status: 200, body: npy([10], biases) };
  }
  return stubs;
})();

/** Demos and what their probed frame shows. */
const DEMOS: Record<string, Expectation> = {
  HelloTriangle: { colors: [{ rgba: "#0000", min: 50 }, { rgba: "#f00f", min: 10 }] },
  HelloTriangleMSAA: { colors: [{ rgba: "#000f", min: 50 }, { rgba: "#f00f", min: 10 }], distinct: 3 },
  ImageBlur: { distinct: 100 },
  ComputeToys: { network: TOY_NETWORK, distinct: 50 },
  Cube: { colors: [{ rgba: "#0000", min: 40 }], distinct: 100 },
  CanvasAPI: {
    colors: [{ rgba: "#000f", min: 50 }, { rgba: "#f00f", min: 10 }],
    before: async (app) => {
      await app.wait(200);
      await app.press("id:check-surface");
    },
  },
  Cubemap: { distinct: 100 },
  InstancedCube: { colors: [{ rgba: "#888f", min: 50 }], distinct: 100 },
  TexturedCube: { colors: [{ rgba: "#888f", min: 40 }], distinct: 200 },
  FractalCube: { colors: [{ rgba: "#888f", min: 40 }], distinct: 20 },
  Particles: { colors: [{ rgba: "#000f", min: 50 }, { rgba: "#ffff", min: 3 }], distinct: 20 },
  RenderBundles: { colors: [{ rgba: "#000f", min: 50 }], distinct: 50 },
  // Its camera turns with Date.now() / 1000 from the first frame: the clock
  // starts at the epoch so the first frame looks the same every run.
  OcclusionQuery: { clock: { time: 0 }, colors: [{ rgba: "#888f", min: 60 }], distinct: 2 },
  ReversedZ: {
    colors: [{ rgba: "#008f", min: 60 }, { rgba: "#f00f", min: 5 }, { rgba: "#0f0f", min: 3 }],
    distinct: 3,
  },
  ComputeBoids: { colors: [{ rgba: "#0000", min: 70 }], distinct: 30 },
  MNISTInference: { network: MNIST_NETWORK, colors: [{ rgba: "#eeff", min: 70 }, { rgba: "#dddf", min: 4 }] },
  ShadowMapping: { colors: [{ rgba: "#333f", min: 20 }], distinct: 100 },
  SamplerParameters: { colors: [{ rgba: "#333f", min: 40 }], distinct: 100 },
  DeferedRendering: { colors: [{ rgba: "#333f", min: 20 }], distinct: 100 },
  ABuffer: { colors: [{ rgba: "#000f", min: 40 }], distinct: 30 },
  Wireframe: { colors: [{ rgba: "#444f", min: 20 }], distinct: 200 },
  Resize: { colors: [{ rgba: "#8888", min: 30 }, { rgba: "#f00f", min: 3 }] },
  // 16 tiles of equal area: which five are "most common" is arbitrary.
  GradientTiles: { colors: [{ rgba: "#888f", min: 5, tolerance: 8 }], distinct: 16 },
  // Random sizes and colors on every run: only the count of colors is stable.
  StorageBufferVertices: { distinct: 20 },
  // createTestVideoFrame's pattern, sampled through a GPUExternalTexture.
  // wgpu 29 has external textures on Metal and DX12, not on Vulkan or GL.
  ImportExternalTexture: {
    distinct: 100,
    unsupported: /ERR_WEBGPU_UNSUPPORTED: importExternalTexture on a device without wgpu's EXTERNAL_TEXTURE feature/,
  },
  SixteenBitTextures: { colors: [{ rgba: "#334f", min: 20 }] },
};

/**
 * Where the manual clock starts: demos turn their camera with Date.now(),
 * so a fixed start makes their first frame the same on every run (the
 * reference Cube specs' time).
 */
const START = 1721677648 * 1000;

/** The whole tour's time limit: every demo's setup and readback run on the machine's GPU. */
const TIMEOUT = 5 * 60_000;

/** The reference's Home entries in order (apps/example/src/Home.tsx, with its two commented-out ones in place). */
const HOME = [
  "Tests",
  "HelloTriangle",
  "HelloTriangleMSAA",
  "ImageBlur",
  "Reanimated",
  "FramePacing",
  "ComputeToys",
  "ThreeJS",
  "Tensorflow",
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
  "Diagnostics",
  "StorageBufferVertices",
  "SharedTextureMemory",
  "ImportExternalTexture",
  "VisionCamera",
  "SixteenBitTextures",
  "ViewSnapshot",
  "NativeBufferView",
  "IconBenchmark",
];

/** Entries that are explanation pages. */
const SKIPPED = [
  "Tests",
  "Reanimated",
  "FramePacing",
  "ThreeJS",
  "Tensorflow",
  "SharedTextureMemory",
  "VisionCamera",
  "ViewSnapshot",
  "NativeBufferView",
  "IconBenchmark",
];

/** The Diagnostics list's entries that are explanation pages. */
const SKIPPED_DIAGNOSTICS = ["WorkletRequestAdapter", "ReloadLifecycle", "TransparencyMode"];

/** Snapshot mode: demo -> most pixels allowed to differ (of 1024²). */
const SNAPSHOTS: Record<string, number> = {
  HelloTriangle: 200,
  HelloTriangleMSAA: 200,
  ImageBlur: 200,
  Cube: 200,
  InstancedCube: 200,
  TexturedCube: 200,
  FractalCube: 200,
};

interface Report {
  line: string;
  width: number;
  height: number;
  errors: number;
  distinct: number;
  top: { rgba: string; share: number }[];
  snapshot?: { differ: number; total: number };
}

function parse(line: string): Report {
  const m = /: (\d+)x(\d+) frame=\d+ err=(\d+) distinct=(\d+) top=(\S+)(?: snapshot=\S+:(\d+)\/(\d+))?$/.exec(line);
  if (!m) throw new Error(`unexpected probe line: ${line}`);
  return {
    line,
    width: Number(m[1]),
    height: Number(m[2]),
    errors: Number(m[3]),
    distinct: Number(m[4]),
    top: m[5].split(",").map((entry) => {
      const [rgba, share] = entry.split(":");
      return { rgba, share: Number(share) };
    }),
    snapshot: m[6] ? { differ: Number(m[6]), total: Number(m[7]) } : undefined,
  };
}

function near(a: string, b: string, tolerance: number): boolean {
  for (let i = 1; i < 5; i++) {
    if (Math.abs(Number.parseInt(a[i], 16) - Number.parseInt(b[i], 16)) > tolerance) return false;
  }
  return true;
}

class ProbeFailure extends Error {}

/**
 * Waits until `done`, on the manual clock: `waitUntil` moves it on every host
 * but the web's (NSH 0.7.1), where only `wait` does.
 */
async function until(app: App, done: () => boolean, timeout: number): Promise<void> {
  const end = Date.now() + timeout;
  for (;;) {
    try {
      await app.waitUntil(done, { timeout: 1000 });
      return;
    } catch (error) {
      if (Date.now() > end || done()) {
        if (done()) return;
        throw error;
      }
    }
    await app.wait(100);
  }
}

async function probe(app: App, screen: string, before?: (app: App) => Promise<void>): Promise<Report> {
  await app.press(`id:open-${screen}`);
  await before?.(app);
  const prefix = `probe ${screen}: `;
  const current = () => app.texts().find((t) => t.startsWith(prefix)) ?? "";
  await until(app, () => / frame=| failed/.test(current()), 120_000);
  const line = current();
  console.log(line);
  await app.back();
  if (line.includes(" failed")) throw new ProbeFailure(line);
  return parse(line);
}

function check(screen: string, report: Report, expectation: Expectation): void {
  expect(report.errors).toBe(0);
  expect(report.width).toBeGreaterThan(0);
  expect(report.distinct).toBeGreaterThanOrEqual(expectation.distinct ?? 2);
  for (const color of expectation.colors ?? []) {
    const found = report.top.find((t) => near(t.rgba, color.rgba, color.tolerance ?? 1));
    if (!found || found.share < color.min) {
      throw new Error(`${screen}: expected ${color.rgba} >= ${color.min}% in ${report.line}`);
    }
  }
}

for (const [screen, expectation] of Object.entries(DEMOS)) {
  test(`${screen}: opens from Home and draws without GPU errors`, async () => {
    const app = await open("./App.tsx", { clock: expectation.clock ?? { time: START }, network: expectation.network });
    if (expectation.failure) {
      let message = "drew";
      try {
        await probe(app, screen, expectation.before);
      } catch (error) {
        if (!(error instanceof ProbeFailure)) throw error;
        message = error.message;
      }
      expect(message).toMatch(expectation.failure);
      return;
    }
    let report: Report;
    try {
      report = await probe(app, screen, expectation.before);
    } catch (error) {
      if (error instanceof ProbeFailure && expectation.unsupported?.test(error.message)) return;
      throw error;
    }
    check(screen, report, expectation);
  }, TIMEOUT);
}

test("Home lists the reference's entries, in its order", async () => {
  const app = await open("./App.tsx", { clock: {} });
  const ids = (app.accessibility("id:list")?.children ?? []).map((node) => node.label ?? "");
  console.log(`${HOME.length} entries expected; accessibility rows: ${ids.length}`);
  // Yields at each row: one call into JS may take 2 s, which a slow runner's host can need for them all.
  for (const screen of HOME) {
    expect(app.count(`id:open-${screen}`)).toBe(1);
    await app.wait(1);
  }
  const order = app.texts();
  let last = -1;
  for (const screen of HOME) {
    const row = app.layout(`id:open-${screen}`);
    if (!row) throw new Error(`no row for ${screen}`);
    expect(row.y).toBeGreaterThan(last);
    last = row.y;
    await app.wait(1);
  }
  expect(order.length).toBeGreaterThan(HOME.length);
}, TIMEOUT);

test("the entries that are not ported open a page saying why", async () => {
  const app = await open("./App.tsx", { clock: {} });
  for (const screen of SKIPPED) {
    await app.press(`id:open-${screen}`);
    expect(app.shows(`id:skip-${screen}`)).toBe(true);
    await app.back();
  }
  await app.press("id:open-Diagnostics");
  for (const screen of SKIPPED_DIAGNOSTICS) {
    await app.press(`id:open-${screen}`);
    expect(app.shows(`id:skip-${screen}`)).toBe(true);
    await app.back();
  }
}, TIMEOUT);

test("MNISTInference: a stroke drawn on the pad runs the network", async () => {
  const app = await open("./App.tsx", { clock: {}, network: MNIST_NETWORK });
  await app.press("id:open-MNISTInference");
  const status = () => app.texts().find((t) => t === "Draw a digit" || t.startsWith("The network did not load")) ?? "";
  await until(app, () => status() !== "", 60_000);
  expect(status()).toBe("Draw a digit");
  await app.drag("id:mnist-pad", { x: 0, y: 200 });
  await until(app, () => app.texts().includes("7"), 60_000);
  await app.press("id:mnist-reset");
  expect(app.texts().includes("7")).toBe(false);
}, TIMEOUT);

/** Diagnostics: what to press, and the log line that must follow. None may log an uncaught GPU error. */
// The diagnostics run on the real clock: they are frame loops left running
// for seconds, and on the macOS host a manual clock's wait runs the frames
// requestAnimationFrame chains within one another's stack until it overflows
// (about 2,200 frames: "Maximum call stack size exceeded"), which is the test
// clock's doing, not the app's.
const DIAGNOSTICS: Record<
  string,
  { press?: string[]; ready?: RegExp; expect: RegExp; never?: RegExp; uncaptured?: RegExp }
> = {
  // Its timers and frames run while the pipeline compiles, off the JS
  // thread. A pipeline the driver has compiled before (a second run) is
  // ready within a millisecond or two, before a timer or a frame is due (at
  // 60 Hz: 16 ms, which is a Windows timer's tick too):
  // the screen then says "blocked" with that time, which is only a real
  // block when it is long (the old JS-thread compile of a cold cache took
  // hundreds of ms). webgpu.test.tsx (examples/headless) checks the
  // off-thread compile with a shader slow to compile.
  AsyncStarvation: { expect: /Event loop remained responsive|Partial responsiveness|Event loop was blocked!.*\((\d|1[0-6]) ms\)/ },
  DeviceLostHang: { expect: /forceLossForTesting is unavailable/ },
  ContextEdgeCases: {
    press: ["edge-unconfigured", "edge-zero", "edge-unconfigure"],
    expect: /after unconfigure\(\)|threw \(correct per spec\).*InvalidStateError/,
    // Chrome's own, for a 0x0 canvas (the crate's is 1x1, as the reference's).
    uncaptured: /texture size \(\[Extent3D width:0, height:0.* is empty|swapchain texture of size 0|\[Invalid Texture/,
  },
  ViewFormatsUseAfterFree: { ready: /^configured /, press: ["viewformats-resize"], expect: /canvas.width -> \d+/ },
  // The loop keeps rendering after the canvas unmounts: the context keeps
  // its configuration and falls back to offscreen rendering, as the
  // reference's. Its frame count passes 120 only after the unmount (the
  // press comes 0.5 s after the loop starts), and no frame throws.
  RenderAfterUnmount: {
    ready: /^rendering, unmount/,
    press: ["unmount-toggle"],
    expect: /^frame ([2-9]\d\d|\d{4,}) ok/,
    never: /^frame \d+: /,
  },
  BackgroundDetach: { expect: /rendering, background the app/ },
  SurfaceChurn: { press: ["churn-toggle"], expect: /^epoch 10$/, never: /^epoch \d+( frame \d+)?: / },
  DeviceDestroyBeforeDetach: { press: ["destroy-toggle"], expect: /\[device lost\] destroyed/ },
};

for (const [screen, diagnostic] of Object.entries(DIAGNOSTICS)) {
  test(`Diagnostics › ${screen}: runs without an uncaught GPU error`, async () => {
    const app = await open("./App.tsx");
    await app.press("id:open-Diagnostics");
    await app.press(`id:open-${screen}`);
    const ready = diagnostic.ready;
    if (ready) await until(app, () => app.texts().some((t) => ready.test(t)), 60_000);
    await app.wait(500);
    for (const id of diagnostic.press ?? []) {
      await app.press(`id:${id}`);
      await app.wait(300);
    }
    await until(app, () => app.texts().some((t) => diagnostic.expect.test(t)), 60_000);
    const lines = app.texts().filter((t) => t.startsWith("[") || /threw|frame|epoch|cleanup|device/.test(t));
    console.log(`${screen}: ${lines.join(" | ")}`);
    const uncaptured = app.texts().filter((t) => t.startsWith("[uncaptured]"));
    expect(uncaptured.filter((t) => !diagnostic.uncaptured?.test(t))).toEqual([]);
    // A device the page destroys is lost as "destroyed"; any other loss is the GPU's.
    expect(app.texts().filter((t) => t.startsWith("[device lost] unknown"))).toEqual([]);
    const never = diagnostic.never;
    if (never) expect(app.texts().filter((t) => never.test(t))).toEqual([]);
  }, TIMEOUT);
}

for (const [screen, limit] of Object.entries(SNAPSHOTS)) {
  test(`${screen}: matches the reference snapshot`, async () => {
    const app = await open("./App.tsx", { clock: {} });
    await app.press("id:snapshot-mode");
    const report = await probe(app, screen);
    expect(report.errors).toBe(0);
    if (!report.snapshot) throw new Error(`${screen}: no snapshot comparison in ${report.line}`);
    if (report.snapshot.differ > limit) {
      throw new Error(`${screen}: ${report.snapshot.differ} pixels differ from the snapshot (limit ${limit})`);
    }
  }, TIMEOUT);
}
