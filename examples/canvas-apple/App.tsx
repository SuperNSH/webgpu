// The canvas on screen: the reference's Triangle (one frame) and Cube (a
// requestAnimationFrame loop) side by side, each in a <Canvas> whose drawing
// buffer is its layout size times the pixel ratio, as the reference's
// example sizes it.

import { cleanup, mount, pixelRatio, platform, signal, Text, View } from "nsh";
import { Canvas, type CanvasRef } from "nsh-webgpu";
import { copyOut, logPng } from "./capture";
import { drawTriangle, runCube, type Tap } from "./scenes";

/** The cube frame whose canvas texture is captured. */
const CAPTURED_FRAME = 60;

function App() {
  const status = signal("starting");
  const cubeStatus = signal("cube: waiting");
  // Each canvas hands over its ref once it is mounted and laid out.
  const mounted = (): [Promise<CanvasRef>, (ref: CanvasRef | null) => void] => {
    let resolve: (ref: CanvasRef) => void = () => {};
    const ready = new Promise<CanvasRef>((r) => {
      resolve = r;
    });
    return [ready, (ref) => ref && resolve(ref)];
  };
  const [triangleReady, triangleRef] = mounted();
  const [cubeReady, cubeRef] = mounted();
  let stop: (() => void) | undefined;
  cleanup(() => stop?.());

  (async () => {
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error("No adapter");
    const device = await adapter.requestDevice();
    const format = navigator.gpu.getPreferredCanvasFormat();
    const [triangle, cube] = await Promise.all([triangleReady, cubeReady]);

    const t = triangle.getContext("webgpu")!;
    t.canvas.width = t.canvas.clientWidth * pixelRatio();
    t.canvas.height = t.canvas.clientHeight * pixelRatio();
    // Each canvas's texture is captured once, before it is presented, and
    // logged as a PNG (capture.ts), with whether a view's surface had it.
    const capture =
      (name: string, ref: CanvasRef): Tap =>
      (encoder, texture) => {
        const surface = ref.getNativeSurface().surface !== 0n ? "surface" : "offscreen";
        // Android's GL surfaces cannot be copied from (no COPY_SRC): the
        // copy would invalidate the frame's commands, so nothing is captured.
        if (surface === "surface" && platform.os === "android") return;
        const read = copyOut(device, encoder, texture);
        // Read once the frame is submitted (the tap runs before submit()).
        setTimeout(() => read().then((image) => logPng(`${name}-${surface}`, image)), 0);
      };
    drawTriangle(device, t, format, capture("triangle", triangle));

    const c = cube.getContext("webgpu")!;
    c.canvas.width = c.canvas.clientWidth * pixelRatio();
    c.canvas.height = c.canvas.clientHeight * pixelRatio();
    stop = runCube(
      device,
      c,
      format,
      ({ path, frames }) => cubeStatus.set(`cube (${path}): ${frames} frames`),
      (frame) => (frame === CAPTURED_FRAME ? capture("cube", cube) : undefined),
    );
    status.set(
      `${format} triangle ${t.canvas.width}x${t.canvas.height} cube ${c.canvas.width}x${c.canvas.height} ` +
        `surface ${triangle.getNativeSurface().surface !== 0n ? "attached" : "none"}`,
    );
  })().catch((error: unknown) => status.set(`failed: ${String(error)}`));

  return (
    <View style={{ flex: 1, backgroundColor: "#ffffff", padding: 16, gap: 12 }}>
      <Text style={{ color: "#1f2328" }}>{`WebGPU canvas: ${status()}`}</Text>
      <Text style={{ color: "#1f2328" }}>{cubeStatus()}</Text>
      <View style={{ flex: 1, flexDirection: "row", gap: 12 }}>
        <View style={{ flex: 1, backgroundColor: "#3498db" }}>
          <Canvas ref={triangleRef} style={{ flex: 1 }} />
        </View>
        <View style={{ flex: 1, backgroundColor: "rgb(90, 180, 255)" }}>
          <Canvas ref={cubeRef} style={{ flex: 1 }} />
        </View>
      </View>
    </View>
  );
}

mount(App);
