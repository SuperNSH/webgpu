// apps/example/src/Diagnostics/BackgroundDetach.tsx on NSH: AppState is
// NSH's `lifecycle()`.

import { cleanup, effect, lifecycle, signal } from "nsh";
import { Canvas, requestAnimationFrame } from "nsh-webgpu";
import { SettingRow } from "../../ui";

import { DiagnosticScreen, drawClearFrame, initGPU, useDiagnosticLog } from "./surfaceLifecycle";

// Backgrounding the app destroys the native surface while the JS context
// stays alive. This screen renders continuously; background the app, wait a
// second, and come back. Primarily an Android repro (iOS keeps the
// CAMetalLayer alive in the background).
//
// Flipping the switch while rendering tears down the current native view the
// same way backgrounding does, so it reproduces the TextureView teardown
// without leaving the app.
export const BackgroundDetach = () => {
  const { log, append } = useDiagnosticLog();
  const surfaceView = signal(false);
  let running = true;
  let started = false;
  let gpuDevice: GPUDevice | undefined;
  cleanup(() => {
    running = false;
    gpuDevice?.destroy();
  });

  let first = true;
  effect(() => {
    const state = lifecycle();
    if (first) {
      first = false;
      return;
    }
    append(`AppState: ${state}`);
  });

  return (
    <DiagnosticScreen
      description={
        "Background the app and return: the animated gradient must resume. On a broken build the TextureView " +
        "canvas stays black forever (Android), and the SurfaceView one logs a validation error on resume."
      }
      controls={
        <SettingRow
          testID="background-surfaceview"
          label="SurfaceView"
          detail="Android; the default is a TextureView"
          checked={surfaceView()}
          onChange={(v) => surfaceView.set(v)}
        />
      }
      canvas={
        <Canvas
          ref={(canvas) => {
            if (!canvas || started) return;
            started = true;
            let frame = 0;
            (async () => {
              const { device, format } = await initGPU(append);
              gpuDevice = device;
              const ctx = canvas.getContext("webgpu")!;
              ctx.configure({ device, format, alphaMode: "premultiplied" });
              append("rendering, background the app and come back (or flip the switch)");
              const loop = () => {
                if (!running) {
                  return;
                }
                try {
                  drawClearFrame(device, ctx, frame++);
                  if (frame % 120 === 0) {
                    append(`frame ${frame} ok`);
                  }
                } catch (e) {
                  append(`frame ${frame}: ${e}`);
                }
                requestAnimationFrame(loop);
              };
              loop();
            })().catch((e: unknown) => append(`${e}`));
          }}
          style={{ flex: 1 }}
          android={{ backingView: surfaceView() ? "SurfaceView" : "TextureView" }}
        />
      }
      log={log()}
    />
  );
};
