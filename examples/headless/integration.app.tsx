// The integration checks that need an app of their own (phase 3): `<Canvas>`
// and `installWebGPU()` on the platform the test opens it as. On the web the
// canvas is the browser's (`./surface/web`) and `navigator.gpu` is left to
// the browser; elsewhere it is nsh-webgpu's.

import { mount, platform, Text, View } from "nsh";
import { Canvas, GPU, useCanvasRef } from "nsh-webgpu";

function App() {
  // Set once the canvas is mounted and laid out (a signal: the texts follow).
  const canvas = useCanvasRef();
  const gpu = (globalThis as { navigator?: { gpu?: unknown } }).navigator?.gpu;
  return (
    <View class="flex-1">
      <Text>{`context id: ${canvas.current?.getContextId() ?? "none"}`}</Text>
      <Text>{`surface: ${canvas.current ? String(canvas.current.getNativeSurface().surface) : "none"}`}</Text>
      <View class="flex-1 gap-2 p-4">
        <Text>{`os: ${platform.os}`}</Text>
        <Text>{`navigator.gpu: ${gpu instanceof GPU ? "nsh-webgpu" : gpu === undefined ? "none" : "other"}`}</Text>
        <Canvas style={{ flex: 1 }} ref={canvas} />
      </View>
    </View>
  );
}

mount(App);
