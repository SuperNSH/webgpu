// The Home list: react-native-webgpu's apps/example/src/Home.tsx, entry for
// entry and in its order (its two commented-out entries, Compute Toys and
// Canvas API, are listed where they sit, since they are ported). Each entry
// opens a ported demo, a list (Diagnostics), or a page saying why the demo
// is not ported (docs/parity.md, docs/inventory.md table 4).
//
// The reference hides Shadow Mapping and Sampler Parameters on iOS and
// VisionCamera on macOS; here every entry shows on every platform.

import type { JSX } from "nsh";
import type { ProbeOptions } from "./harness/probe";
import { HelloTriangle } from "./demos/Triangle/HelloTriangle";
import { HelloTriangleMSAA } from "./demos/Triangle/HelloTriangleMSAA";
import { ImageBlur } from "./demos/ImageBlur/ImageBlur";
import { ComputeToys } from "./demos/ComputeToys/ComputeToys";
import { Cube } from "./demos/Cube/Cube";
import { CanvasAPI } from "./demos/CanvasAPI/CanvasAPI";
import { Cubemap } from "./demos/Cube/Cubemap";
import { InstancedCube } from "./demos/Cube/InstancedCube";
import { TexturedCube } from "./demos/Cube/TexturedCube";
import { FractalCube } from "./demos/Cube/FractalCube";
import { Particules } from "./demos/Particles/Particles";
import { RenderBundles } from "./demos/RenderBundles/RenderBundles";
import { OcclusionQuery } from "./demos/OcclusionQuery/OcclusionQuery";
import { ReversedZ } from "./demos/ReversedZ/ReversedZ";
import { ComputeBoids } from "./demos/ComputeBoids/ComputeBoids";
import { MNISTInference } from "./demos/MNISTInference/MNISTInference";
import { ShadowMapping } from "./demos/ShadowMapping/ShadowMapping";
import { SamplerParameters } from "./demos/Sampler/SamplerParameters";
import { DeferedRendering } from "./demos/ShadowMapping/DeferedRendering";
import { ABuffer } from "./demos/ABuffer/ABuffer";
import { Wireframe } from "./demos/Wireframe/Wireframe";
import { Resize } from "./demos/Resize/Resize";
import { GradientTiles } from "./demos/GradientTiles/GradientTiles";
import { diagnostics } from "./demos/Diagnostics/DiagnosticsList";
import { StorageBufferVertices } from "./demos/StorageBufferVertices/StorageBufferVertices";
import { ImportExternalTexture } from "./demos/ImportExternalTexture/ImportExternalTexture";
import { SixteenBitTextures } from "./demos/SixteenBitTextures/SixteenBitTextures";
import triangleSnapshot from "../snapshots/triangle.png?url";
import triangleMsaaSnapshot from "../snapshots/triangle-msaa.png?url";
import cubeSnapshot from "../snapshots/cube.png?url";
import texturedCubeSnapshot from "../snapshots/textured-cube.png?url";
import instancedCubesSnapshot from "../snapshots/instanced-cubes.png?url";
import fractalCubesSnapshot from "../snapshots/fractal-cubes.png?url";
import blurSnapshot from "../snapshots/blur.png?url";

export type Entry =
  | {
      kind: "demo";
      screen: string;
      title: string;
      component: () => JSX.Element;
      probe?: ProbeOptions;
      /** A diagnostic screen: its log is its report, no frame is probed. */
      diagnostic?: boolean;
    }
  | { kind: "list"; screen: string; title: string; entries: Entry[] }
  | { kind: "skip"; screen: string; title: string; reason: string };

const skip = (screen: string, title: string, reason: string): Entry => ({ kind: "skip", screen, title, reason });
const demo = (screen: string, title: string, component: () => JSX.Element, probe?: ProbeOptions): Entry => ({
  kind: "demo",
  screen,
  title,
  component,
  probe,
});

/** The reference Cube specs' time (packages/webgpu/src/__tests__/demos/Cube.spec.ts), in milliseconds. */
const CUBE_SPEC_TIME = 1721677648 * 1000;
const GRAY: [number, number, number, number] = [0.5, 0.5, 0.5, 1];

export const examples: Entry[] = [
  skip(
    "Tests",
    "🧪 E2E Tests",
    "The reference's end-to-end harness: a screen that evaluates test code sent over a WebSocket by its Jest client. " +
      "Here nsh test plays that role: gallery.test.tsx opens every demo through this list and reads its frame back, " +
      "and the crate's own tests (tests, examples/headless) port the reference's specs.",
  ),
  demo("HelloTriangle", "🔺 Hello Triangle", HelloTriangle, {
    snapshot: { file: "triangle.png", url: triangleSnapshot, clear: [0.3, 0.6, 1, 1] },
  }),
  demo("HelloTriangleMSAA", "🔺 Hello Triangle MSAA", HelloTriangleMSAA, {
    snapshot: { file: "triangle-msaa.png", url: triangleMsaaSnapshot, clear: [0, 0, 0, 1] },
  }),
  demo("ImageBlur", "🫧 Image Blur", ImageBlur, {
    snapshot: { file: "blur.png", url: blurSnapshot, clear: [0, 0, 0, 1] },
  }),
  skip(
    "Reanimated",
    "🐎 Reanimated",
    "Renders from react-native-reanimated's UI thread and dedicated worklet runtimes, and from a VisionCamera frame " +
      "processor. NSH has no worklet runtimes: a canvas renders from the JS thread, and wgpu's objects are Send + Sync, " +
      "so Rust code can render from any thread instead.",
  ),
  skip(
    "FramePacing",
    "⏱️ Frame Pacing",
    "Measures requestAnimationFrame on the JS, UI and dedicated worklet runtimes with a 'JS work' slider; it needs " +
      "react-native-reanimated and react-native-worklets. NSH's requestAnimationFrame is its frame clock's, one " +
      "runtime only.",
  ),
  demo("ComputeToys", "🧮 Compute Toys", ComputeToys),
  skip(
    "ThreeJS",
    "☘️ Three.js",
    "three.js's WebGPURenderer, its loaders (glTF, DRACO, HDR) and @react-three/fiber are npm packages; NSH's " +
      "bundler resolves none, and three.js expects a DOM. Not ported.",
  ),
  skip(
    "Tensorflow",
    "🤖 tensorflow.js",
    "@tensorflow/tfjs and its WebGPU backend are npm packages, which NSH's bundler does not resolve. Not ported.",
  ),
  demo("Cube", "🧊 Cube", Cube, {
    snapshot: { file: "cube.png", url: cubeSnapshot, time: CUBE_SPEC_TIME, clear: GRAY },
  }),
  demo("CanvasAPI", "🧩 Canvas API", CanvasAPI),
  demo("Cubemap", "🌉 Cubemap", Cubemap),
  demo("InstancedCube", "🎲 Instanced Cube", InstancedCube, {
    snapshot: { file: "instanced-cubes.png", url: instancedCubesSnapshot, time: CUBE_SPEC_TIME, clear: GRAY },
  }),
  demo("TexturedCube", "🥷 Textured Cube", TexturedCube, {
    snapshot: { file: "textured-cube.png", url: texturedCubeSnapshot, time: CUBE_SPEC_TIME, clear: GRAY },
  }),
  demo("FractalCube", "❄️ Fractal Cube", FractalCube, {
    frame: 30,
    snapshot: {
      file: "fractal-cubes.png",
      url: fractalCubesSnapshot,
      time: CUBE_SPEC_TIME,
      step: 16_000,
      frames: 9,
      clear: GRAY,
    },
  }),
  demo("Particles", "🌌 Particles", Particules, { frame: 30 }),
  demo("RenderBundles", "🪐 Render Bundles", RenderBundles),
  demo("OcclusionQuery", "🟩 Occlusion Query", OcclusionQuery),
  demo("ReversedZ", "🟥 Reversed Z", ReversedZ),
  demo("ComputeBoids", "🐦‍⬛ Compute Boids", ComputeBoids),
  demo("MNISTInference", "1️⃣ MNIST Inference", MNISTInference),
  demo("ShadowMapping", "🐲 Shadow Mapping", ShadowMapping),
  demo("SamplerParameters", "🏁 Sampler Parameters", SamplerParameters),
  demo("DeferedRendering", "🚦 Deferred Rendering", DeferedRendering),
  demo("ABuffer", "🫖 A-Buffer", ABuffer),
  demo("Wireframe", "🧬 Wireframe", Wireframe),
  demo("Resize", "↔️ Resize", Resize, { frame: 60 }),
  demo("GradientTiles", "🌈 Gradient Tiles", GradientTiles),
  { kind: "list", screen: "Diagnostics", title: "⚠️ Tests", entries: diagnostics },
  demo("StorageBufferVertices", "💾 Storage Buffer Vertices", StorageBufferVertices),
  skip(
    "SharedTextureMemory",
    "🎞️ Shared Texture Memory",
    "Decodes video (AVPlayer) or a test frame into Dawn's shared texture memory and samples it. wgpu has no shared " +
      "texture memory (importing an IOSurface or AHardwareBuffer needs wgpu-hal interop), so createVideoPlayer and " +
      "createNativeBuffer throw ERR_WEBGPU_UNSUPPORTED.",
  ),
  demo("ImportExternalTexture", "🎬 Import External Texture", ImportExternalTexture),
  skip(
    "VisionCamera",
    "📷 VisionCamera integration",
    "Live camera frames through react-native-vision-camera and its worklets: a React Native camera SDK with no NSH " +
      "equivalent, and zero-copy frame import that wgpu cannot do.",
  ),
  demo("SixteenBitTextures", "🎨 16bits Textures", SixteenBitTextures),
  skip(
    "ViewSnapshot",
    "📸 View Snapshot (drawElementImageToTexture)",
    "Draws a React Native view into a texture with GPUQueue.drawElementImageToTexture. NSH has no hook that renders " +
      "a view to pixels for another library, so the method is not implemented.",
  ),
  skip(
    "NativeBufferView",
    "🖼️ Native Buffer View",
    "Renders into native buffers (IOSurface, AHardwareBuffer) presented by a NativeBufferView. wgpu cannot render " +
      "into an imported platform buffer, and NSH has no view that displays one: NativeBufferView throws " +
      "ERR_WEBGPU_UNSUPPORTED.",
  ),
  skip(
    "IconBenchmark",
    "⏱️ 200 Icons: Skia Canvas vs NativeBufferView",
    "Compares react-native-skia's Canvas with NativeBufferView. Neither exists here: Skia is a React Native library, " +
      "and NativeBufferView is unsupported (see Native Buffer View).",
  ),
];

/** Every entry, lists flattened, by screen name. */
export const byScreen = new Map<string, Entry>();
const index = (entries: Entry[]) => {
  for (const entry of entries) {
    byScreen.set(entry.screen, entry);
    if (entry.kind === "list") index(entry.entries);
  }
};
index(examples);
