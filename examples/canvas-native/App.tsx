// The `webgpu.canvas` platform view on screen (Android, Windows, Linux):
// the view nsh-webgpu registers, given a context id as `<Canvas>` gives it,
// drawn into by the example's own crate (`CanvasDemo.draw`) straight on
// the surface the view attached. A triangle over a clear color that cycles.

import { mount, nativeComponent, signal, Text, View } from "nsh";
import { CanvasDemo } from "canvas/native";

// As `<Canvas>` declares the view (props: the reference's resolved ones).
const WebGPUView = nativeComponent<{
  contextId: number;
  androidBackingView?: "TextureView" | "SurfaceView" | "HardwareBufferView";
  androidOpaque?: boolean;
}>("webgpu.canvas", { props: ["contextId", "androidBackingView", "androidOpaque"] });

function Demo(props: { contextId: number; backing: "TextureView" | "SurfaceView" }) {
  const status = signal("starting");
  let frame = 0;
  setInterval(() => {
    try {
      status.set(CanvasDemo.draw(props.contextId, frame++));
    } catch (error) {
      status.set(`failed: ${String(error)}`);
    }
  }, 50);
  return (
    <View class="flex-1 gap-2">
      <Text class="text-foreground" testID={`status-${props.contextId}`}>
        {props.backing}: {status()}
      </Text>
      <WebGPUView
        contextId={props.contextId}
        androidBackingView={props.backing}
        androidOpaque={props.backing === "SurfaceView"}
        style={{ flex: 1 }}
      />
    </View>
  );
}

function App() {
  return (
    <View class="flex-1 gap-4 bg-background p-4">
      <Demo contextId={1} backing="TextureView" />
      <Demo contextId={2} backing="SurfaceView" />
    </View>
  );
}

mount(App);
