// The <Canvas> warnings the reference gives in development: opened as an
// Android app by canvas-lifecycle.test.tsx, which reads what
// console.warn printed as texts.

import { mount, signal, Text, View } from "nsh";
import { Button } from "nsh/base";
import { type AndroidCanvasProps, Canvas } from "nsh-webgpu";

// What console.warn printed, shown as texts for the test.
const warnings = signal<string[]>([]);
const warn = console.warn;
console.warn = (...args: unknown[]) => {
  warnings.set([...warnings(), args.join(" ")]);
  warn(...args);
};

function App() {
  const android = signal<AndroidCanvasProps>({ backingView: "HardwareBufferView" });
  return (
    <View class="flex-1 gap-2 p-4">
      <Button testID="surface-view" onPress={() => android.set({ backingView: "SurfaceView", zOrderOnTop: true })}>
        <Text>surface view</Text>
      </Button>
      <Canvas android={android()} style={{ width: 20, height: 20 }} />
      {warnings().map((line) => (
        <Text>{`warned: ${line}`}</Text>
      ))}
    </View>
  );
}

mount(App);
