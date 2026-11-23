// apps/example/src/Diagnostics/ViewFormatsUseAfterFree.tsx on NSH.

import { cleanup, pixelRatio } from "nsh";
import type { RNCanvasContext } from "nsh-webgpu";
import { Canvas, requestAnimationFrame } from "nsh-webgpu";
import { contextAfterLayout } from "../../harness/useWebGPU";
import { ActionButton } from "../../ui";

import { DiagnosticScreen, drawClearFrame, initGPU, useDiagnosticLog } from "./surfaceLifecycle";

// configure() copies the wgpu::SurfaceConfiguration into SurfaceInfo, but the
// viewFormats array inside it is owned by the Convertor that only lives for
// the duration of the configure() call. The stored configuration keeps a
// pointer into that freed memory. Any later reconfigure re-reads it:
// - a canvas resize (this screen's button),
// - a device rotation,
// - the offscreen/onscreen transitions on background/foreground.
//
// The button churns the native heap first (shader module allocations) to
// poison the freed allocation, then shrinks canvas.width by one pixel so the
// next getCurrentTexture() triggers the reconfigure path.
export const ViewFormatsUseAfterFree = () => {
  const { log, append } = useDiagnosticLog();
  let gpu: { device: GPUDevice; context: RNCanvasContext } | null = null;
  let running = true;
  let started = false;
  cleanup(() => {
    running = false;
    gpu?.device.destroy();
  });

  const resize = () => {
    if (!gpu) {
      return;
    }
    const { device, context } = gpu;
    // Churn the native heap so the freed viewFormats allocation gets reused
    // before the reconfigure reads it.
    for (let i = 0; i < 32; i++) {
      device.createShaderModule({
        code: `/* ${"poison".repeat(256 + i)} */
@vertex fn v() -> @builtin(position) vec4f { return vec4f(0); }`,
      });
    }
    const canvas = context.canvas;
    canvas.width -= 1;
    append(`canvas.width -> ${canvas.width}, next frame reconfigures with the dangling viewFormats pointer`);
  };

  return (
    <DiagnosticScreen
      description={
        "The canvas is configured with viewFormats. Resizing forces a native reconfigure that re-reads the " +
        "viewFormats array configure() was given; on a broken build that memory was freed when configure() returned."
      }
      controls={
        <ActionButton
          testID="viewformats-resize"
          label="Resize canvas (reconfigure with dangling viewFormats)"
          onPress={resize}
        />
      }
      canvas={
        <Canvas
          ref={(canvasRef) => {
            if (!canvasRef || started) return;
            started = true;
            let frame = 0;
            (async () => {
              const { device, format } = await initGPU(append);
              const ctx = await contextAfterLayout(canvasRef, () => !running);
              if (!ctx) return;
              const canvas = ctx.canvas;
              canvas.width = canvas.clientWidth * pixelRatio();
              canvas.height = canvas.clientHeight * pixelRatio();
              const viewFormat: GPUTextureFormat = `${format}-srgb` as GPUTextureFormat;
              ctx.configure({
                device,
                format,
                alphaMode: "opaque",
                viewFormats: [viewFormat],
              });
              append(`configured ${format} with viewFormats: [${viewFormat}]`);
              gpu = { device, context: ctx };
              const loop = () => {
                if (!running) {
                  return;
                }
                try {
                  drawClearFrame(device, ctx, frame++);
                } catch (e) {
                  append(`frame ${frame}: ${e}`);
                }
                requestAnimationFrame(loop);
              };
              loop();
            })().catch((e: unknown) => append(`${e}`));
          }}
          style={{ flex: 1 }}
        />
      }
      log={log()}
    />
  );
};
