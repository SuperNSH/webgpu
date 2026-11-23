// apps/example/src/Diagnostics/SurfaceChurn.tsx on NSH. The reference
// remounts <Canvas key={epoch}>; here the canvas is rebuilt by a binding
// that reads `epoch`, and each new canvas's ref starts that epoch's loop.

import { cleanup, effect, signal, View } from "nsh";
import type { CanvasRef } from "nsh-webgpu";
import { Canvas, requestAnimationFrame } from "nsh-webgpu";
import { ActionButton } from "../../ui";

import { type DiagnosticGPU, DiagnosticScreen, drawClearFrame, initGPU, useDiagnosticLog } from "./surfaceLifecycle";

// Automated stress for the attach/detach transitions. While a frame loop
// renders on the JS thread, the canvas is fully remounted every 400ms (every
// third remount also flips the Android backing view between a TextureView
// and a SurfaceView). Each epoch tears the previous native view down on the platform
// thread while the previous frame may still be in flight, exercising:
// - native teardown (MetalView dealloc / TextureView destroy) racing
//   getCurrentTexture()/present() on the JS thread,
// - registry add/remove churn against context creation,
// - repeated offscreen/onscreen transitions and reconfigures.
//
// Expected: the gradient flickers but the app survives indefinitely with no
// validation errors. On a broken build this crashes within seconds
// (use-after-free of the native layer/window) or degrades into a black canvas
// and a stream of validation errors.
export const SurfaceChurn = () => {
  const { log, append } = useDiagnosticLog();
  const running = signal(false);
  const epoch = signal(0);
  const deviceRef: { current: DiagnosticGPU | null } = { current: null };
  let live: { current: boolean } = { current: false };
  cleanup(() => {
    live.current = false;
    deviceRef.current?.device.destroy();
  });

  effect(() => {
    if (!running()) {
      live.current = false;
      return;
    }
    const interval = setInterval(() => {
      epoch.update((e) => e + 1);
    }, 400);
    cleanup(() => clearInterval(interval));
  });

  const start = (canvas: CanvasRef, current: number) => {
    live.current = false;
    const mine = { current: true };
    live = mine;
    let frame = 0;
    (async () => {
      if (deviceRef.current === null) {
        deviceRef.current = await initGPU(append);
      }
      const { device, format } = deviceRef.current;
      if (!mine.current) {
        return;
      }
      const ctx = canvas.getContext("webgpu")!;
      ctx.configure({ device, format, alphaMode: "premultiplied" });
      if (current % 10 === 0) {
        append(`epoch ${current}`);
      }
      const loop = () => {
        if (!mine.current) {
          return;
        }
        try {
          drawClearFrame(device, ctx, current * 24 + frame++);
        } catch (e) {
          append(`epoch ${current} frame ${frame}: ${e}`);
        }
        requestAnimationFrame(loop);
      };
      loop();
    })().catch((e: unknown) => append(`epoch ${current}: ${e}`));
  };

  return (
    <DiagnosticScreen
      description={
        "Remounts the canvas every 400ms while rendering every frame, flipping the Android backing view every " +
        "third remount. Expected: flicker, but no crash and no validation errors. Leave it running for a minute."
      }
      controls={
        <ActionButton
          testID="churn-toggle"
          label={running() ? "Stop churn" : "Start churn"}
          onPress={() => running.update((r) => !r)}
        />
      }
      canvas={
        running() ? (
          (() => {
            const current = epoch();
            return (
              <Canvas
                ref={(canvas) => {
                  if (canvas) start(canvas, current);
                }}
                style={{ flex: 1 }}
                android={{ backingView: Math.floor(current / 3) % 2 !== 0 ? "SurfaceView" : "TextureView" }}
              />
            );
          })()
        ) : (
          <View style={{ flex: 1 }} />
        )
      }
      log={log()}
    />
  );
};
