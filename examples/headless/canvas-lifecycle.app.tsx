// The <Canvas> ref's lifecycle through the UI, headless: when the ref is
// set (after the view's first layout, not during render), that it gets
// `null` on unmount, that the old ref's getContext() then throws, and
// that useSurface() follows the canvas's layout. canvas-lifecycle.test.tsx
// reads the texts `<name>: <value>`.

import { mount, signal, Text, View } from "nsh";
import { Button } from "nsh/base";
import { Canvas, type CanvasRef, useSurface } from "nsh-webgpu";

function App() {
  const shown = signal(true);
  const width = signal(40);
  const refs = signal<string[]>([]);
  const firstContext = signal("none");
  const stale = signal("not tried");
  // NSH compiles `ref={objectRef}` to an assignment of the variable.
  let objectRef = null as CanvasRef | null;
  const objectRefNow = signal("unset");
  const hooked = useSurface();
  let rendering = true;
  let last: CanvasRef | null = null;

  const thrown = (f: () => unknown): string => {
    try {
      f();
      return "none";
    } catch (error) {
      return (error as Error).message;
    }
  };

  const canvasRef = (canvas: CanvasRef | null) => {
    refs.set([...refs(), canvas === null ? "null" : rendering ? "canvas during render" : "canvas"]);
    if (canvas) {
      last = canvas;
      const context = canvas.getContext("webgpu");
      if (firstContext() === "none" && context) {
        firstContext.set(`${context.canvas.width}x${context.canvas.height}`);
      }
    }
  };

  const element = (
    <View class="flex-1 gap-2 p-4">
      <Button testID="toggle" onPress={() => shown.update((s) => !s)}>
        <Text>toggle</Text>
      </Button>
      <Button testID="wider" onPress={() => width.set(56)}>
        <Text>wider</Text>
      </Button>
      <Button
        testID="stale"
        onPress={() => {
          stale.set(last ? thrown(() => last!.getContext("webgpu")) : "no ref");
          objectRefNow.set((objectRef as CanvasRef | null) === null ? "null" : "canvas");
        }}
      >
        <Text>stale</Text>
      </Button>
      <View style={{ width: width(), height: 30 }}>
        {shown() ? <Canvas ref={canvasRef} style={{ flex: 1 }} /> : <View style={{ flex: 1 }} />}
      </View>
      <View style={{ width: width(), height: 20 }}>
        {shown() ? <Canvas ref={hooked.ref} style={{ flex: 1 }} /> : <View style={{ flex: 1 }} />}
      </View>
      <View style={{ width: 10, height: 10 }}>
        {shown() ? <Canvas ref={objectRef} style={{ flex: 1 }} /> : <View style={{ flex: 1 }} />}
      </View>
      <Text>{`refs: ${refs().join(",")}`}</Text>
      <Text>{`first context: ${firstContext()}`}</Text>
      <Text>{`surface: ${hooked.surface ? `${hooked.surface.width}x${hooked.surface.height}` : "none"}`}</Text>
      <Text>{`client: ${hooked.surface ? `${hooked.surface.clientWidth}x${hooked.surface.clientHeight}` : "none"}`}</Text>
      <Text>{`stale getContext: ${stale()}`}</Text>
      <Text>{`object ref: ${objectRefNow()}`}</Text>
    </View>
  );
  rendering = false;
  return element;
}

mount(App);
