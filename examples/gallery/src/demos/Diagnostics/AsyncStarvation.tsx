// apps/example/src/Diagnostics/AsyncStarvation.tsx on NSH. NSH has no
// requestIdleCallback: a zero-delay timeout stands in (it also runs only once
// the JS thread is free).

import { cleanup, signal, Text, View } from "nsh";
import { cancelAnimationFrame, requestAnimationFrame } from "nsh-webgpu";

const requestIdleCallback = (callback: () => void) => setTimeout(callback, 0);
const cancelIdleCallback = (id: ReturnType<typeof setTimeout>) => clearTimeout(id);

export const AsyncStarvation = () => {
  const status = signal("Requesting GPU device…");
  const result = signal<string | null>(null);
  const testPassed = signal<boolean | null>(null);

  let cancelled = false;
  let rafFired = false;
  let idleFired = false;
  let rafId: number | null = null;
  let idleId: ReturnType<typeof setTimeout> | null = null;
  let gpuDevice: GPUDevice | undefined;

  const run = async () => {
    try {
      const adapter = await navigator.gpu.requestAdapter();
      if (!adapter) {
        if (!cancelled) {
          result.set("Failed to acquire a GPU adapter.");
        }
        return;
      }

      const device = await adapter.requestDevice({
        label: "async-starvation-demo",
      });
      gpuDevice = device;

      if (cancelled) {
        return;
      }

      status.set("Testing async operation impact on the event loop…");

      // Use requestAnimationFrame which is more consistent across platforms
      rafId = requestAnimationFrame(() => {
        rafFired = true;
        if (!cancelled) {
          status.set("Animation frame callback fired ✅");
        }
      });

      // Also test with requestIdleCallback, which only fires once the JS
      // thread is free
      idleId = requestIdleCallback(() => {
        idleFired = true;
      });

      const start = Date.now();
      try {
        await device.createRenderPipelineAsync({
          layout: "auto",
          vertex: {
            module: device.createShaderModule({
              code: `
                @vertex
                fn main(@builtin(vertex_index) vertexIndex : u32) -> @builtin(position) vec4f {
                  var pos = array<vec2f, 3>(
                    vec2f(0.0, 0.5),
                    vec2f(-0.5, -0.5),
                    vec2f(0.5, -0.5)
                  );
                  return vec4f(pos[vertexIndex], 0.0, 1.0);
                }
              `,
            }),
            entryPoint: "main",
          },
          fragment: {
            module: device.createShaderModule({
              code: `
                @fragment
                fn main() -> @location(0) vec4f {
                  return vec4f(1.0, 0.0, 0.0, 1.0);
                }
              `,
            }),
            entryPoint: "main",
            targets: [{ format: navigator.gpu.getPreferredCanvasFormat() }],
          },
          primitive: { topology: "triangle-list" },
        });
      } finally {
        // Clean up callbacks
        if (rafId !== null) {
          cancelAnimationFrame(rafId);
        }
        if (idleId !== null) {
          cancelIdleCallback(idleId);
        }
      }

      if (cancelled) {
        return;
      }

      const elapsed = Date.now() - start;

      // Check if our callbacks fired during the async operation
      if (rafFired && idleFired) {
        testPassed.set(true);
        result.set(
          `✅ Event loop remained responsive! Animation frame and idle callback completed during async operation (${elapsed} ms).\n\nThis indicates the WebGPU async operations are properly non-blocking.`,
        );
      } else if (rafFired && !idleFired) {
        testPassed.set(true);
        result.set(
          `⚠️ Partial responsiveness: Animation frame fired but the idle callback was delayed (${elapsed} ms).\n\nThe render loop is working but the JS thread never became idle.`,
        );
      } else {
        testPassed.set(false);
        result.set(
          `❌ Event loop was blocked! Neither animation frames nor idle callbacks could execute during the async operation (${elapsed} ms).\n\nThis indicates potential performance issues with WebGPU async operations.`,
        );
      }
    } catch (error) {
      if (!cancelled) {
        result.set(`Encountered an error while running the demo: ${error instanceof Error ? error.message : String(error)}`);
      }
    }
  };

  void run();

  cleanup(() => {
    cancelled = true;
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
    }
    if (idleId !== null) {
      cancelIdleCallback(idleId);
    }
    gpuDevice?.destroy();
  });

  return (
    <View class="flex-1 justify-center bg-background p-6">
      <View class="gap-3 rounded-xl bg-card p-5">
        <Text class="text-xl font-semibold text-card-foreground">Async Runner Starvation</Text>
        <Text class="text-card-foreground">{status()}</Text>
        <If when={result()}>
          {(text) => (
            <Text testID="diag-result" class={testPassed() ? "text-primary" : "text-destructive"}>
              {text()}
            </Text>
          )}
        </If>
        <Text class="text-xs text-muted-foreground">
          This test verifies that WebGPU async operations don't block the event loop. We test both
          requestAnimationFrame (rendering) and an idle callback (JS thread idle time) to ensure the app remains
          responsive during GPU operations.
        </Text>
      </View>
    </View>
  );
};
