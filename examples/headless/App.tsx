// Renders the reference's Triangle offscreen through nsh-webgpu and shows
// what it read back: the headless check of the TSX classes on the GPU.

import { mount, signal, Text, View } from "nsh";
import { pixel, redCount, renderTriangle } from "./triangle";

function App() {
  const status = signal("triangle: rendering");
  renderTriangle().then(
    ({ format, rgba }) =>
      status.set(
        `triangle: ${format} center=${pixel(rgba, 32, 32)} corner=${pixel(rgba, 0, 0)} red=${redCount(rgba)}`,
      ),
    (error: unknown) => status.set(`triangle: failed: ${String(error)}`),
  );
  return (
    <View class="flex-1 items-center justify-center bg-background p-4">
      <Text class="text-foreground">{status()}</Text>
    </View>
  );
}

mount(App);
