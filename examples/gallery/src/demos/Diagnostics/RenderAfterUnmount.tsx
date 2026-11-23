// apps/example/src/Diagnostics/RenderAfterUnmount.tsx on NSH.

import { cleanup, signal, View } from "nsh";
import { Canvas, requestAnimationFrame } from "nsh-webgpu";
import { ActionButton } from "../../ui";

import { DiagnosticScreen, drawClearFrame, initGPU, useDiagnosticLog } from "./surfaceLifecycle";

// The frame loop deliberately keeps calling getCurrentTexture()/present()
// after the Canvas is unmounted. This is legal from the JS side: the context
// object is still alive, and the documented behavior for a detached surface
// is to fall back to offscreen rendering.
//
// On a broken build, native view teardown frees the surface out from under
// the context:
// - iOS: MetalView dealloc removes the registry entry while the stored
//   wgpu::Surface still wraps the now-released CAMetalLayer. The next
//   getCurrentTexture() asks the dead layer for a drawable: EXC_BAD_ACCESS.
// - Android (TextureView): onSurfaceTextureDestroyed
//   removes the registry entry without detaching, so the context keeps a
//   wgpu::Surface on a destroyed window: dead-window errors or a crash.
//
// backingView is pinned to TextureView: it is the path that crashes rather
// than silently going black. It is the default, but pinning keeps this test
// on the SurfaceTexture teardown path if that default ever changes.
// HardwareBufferView teardown (freeing the AHardwareBuffer pool)
// is covered by TransparencyMode's "hardware buffer" option with hide/show.
export const RenderAfterUnmount = () => {
  const { log, append } = useDiagnosticLog();
  const mounted = signal(true);
  let running = true;
  let started = false;
  let gpuDevice: GPUDevice | undefined;
  cleanup(() => {
    running = false;
    gpuDevice?.destroy();
  });

  return (
    <DiagnosticScreen
      description={
        "A frame loop renders into the canvas and keeps running after the canvas unmounts. Expected: the context " +
        "detaches and falls back to offscreen rendering. On a broken build the next frame uses the freed native " +
        "surface and crashes."
      }
      controls={
        <ActionButton
          testID="unmount-toggle"
          label={mounted() ? "Unmount canvas" : "Remount canvas"}
          onPress={() => mounted.update((m) => !m)}
        />
      }
      canvas={
        mounted() ? (
          <Canvas
            ref={(canvas) => {
              // The reference's effect runs once, with the first canvas (and
              // the ref gets null when the canvas unmounts).
              if (!canvas || started) return;
              started = true;
              let frame = 0;
              (async () => {
                const { device, format } = await initGPU(append);
                gpuDevice = device;
                const ctx = canvas.getContext("webgpu")!;
                ctx.configure({ device, format, alphaMode: "premultiplied" });
                append("rendering, unmount the canvas while the loop keeps going");
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
            android={{ backingView: "TextureView" }}
          />
        ) : (
          <View style={{ flex: 1 }} />
        )
      }
      log={log()}
    />
  );
};
