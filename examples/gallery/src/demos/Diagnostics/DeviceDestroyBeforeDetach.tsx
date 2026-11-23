// apps/example/src/Diagnostics/DeviceDestroyBeforeDetach.tsx on NSH, its
// "raw" mode: the device is destroyed in the scene's cleanup while the
// native view still holds the surface. The reference's three.js mode
// (WebGPURenderer.dispose()) is not ported: three.js is an npm package NSH's
// bundler does not resolve, and the raw mode reproduces the same call order.

import { cleanup, signal, View } from "nsh";
import type { AndroidCanvasProps, CanvasRef } from "nsh-webgpu";
import { Canvas, requestAnimationFrame } from "nsh-webgpu";
import { ActionButton, SettingRow } from "../../ui";

import { DiagnosticScreen, drawClearFrame, initGPU, useDiagnosticLog } from "./surfaceLifecycle";

// Android crash when the device is destroyed before the native surface is
// released (SIGSEGV, fault addr 0x20):
//
//   dawn::native::vulkan::FencedDeleter::DeleteWhenUnused
//   dawn::native::vulkan::SwapChain::DetachFromSurfaceImpl
//   dawn::native::Surface::~Surface
//   rnwgpu::SurfaceInfo::detach(bool)
//   Java_com_webgpu_WebGPUView_onViewDestroyed
//
// Steps: press "Unmount canvas" (or navigate back). The inner component's
// cleanup destroys the device, then the native view is dropped one frame
// later. Expected: the log shows the device-lost event and the app survives.

interface SceneProps {
  surfaceView: boolean;
  unconfigureFirst: boolean;
  append: (line: string) => void;
}

// Either kind of Android backing view, to tear both down.
const backingView = (surfaceView: boolean): AndroidCanvasProps => ({
  backingView: surfaceView ? "SurfaceView" : "TextureView",
});

const RawScene = (props: SceneProps) => {
  const { append } = props;
  const unconfigureFirst = props.unconfigureFirst;
  let running = true;
  let frame = 0;
  let device: GPUDevice | null = null;
  let ctx: ReturnType<CanvasRef["getContext"]> = null;
  let canvasRef: CanvasRef | null = null;

  cleanup(() => {
    running = false;
    if (!device) {
      return;
    }
    // Mirrors WebGPURenderer.dispose(): the device dies synchronously in
    // the effect cleanup, while the native view (and the swapchain attached
    // to its surface) is still alive.
    if (unconfigureFirst && ctx) {
      append("cleanup: context.unconfigure()");
      ctx.unconfigure();
    }
    append("cleanup: device.destroy() (surface still attached)");
    device.destroy();
    append("cleanup done, the native view is dropped next frame");
  });

  const start = async () => {
    const gpu = await initGPU(append);
    if (!running || !canvasRef) {
      // Unmounted before the device arrived: destroy it right away so the
      // cleanup below still exercises the same order (destroy, then the
      // view is dropped).
      gpu.device.destroy();
      return;
    }
    ({ device } = gpu);
    ctx = canvasRef.getContext("webgpu")!;
    ctx.configure({
      device,
      format: gpu.format,
      alphaMode: "premultiplied",
    });
    append("rendering, now unmount the canvas");
    const loop = () => {
      if (!running || !device || !ctx) {
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
  };

  return (
    <Canvas
      ref={(canvas) => {
        canvasRef = canvas;
        // Null when the canvas unmounts: the cleanup above handles that.
        if (canvas) start().catch((e: unknown) => append(`${e}`));
      }}
      style={{ flex: 1 }}
      android={backingView(props.surfaceView)}
    />
  );
};

export const DeviceDestroyBeforeDetach = () => {
  const { log, append } = useDiagnosticLog();
  const mounted = signal(true);
  const surfaceView = signal(true);
  const unconfigureFirst = signal(false);

  return (
    <DiagnosticScreen
      description={
        "The canvas is configured and rendering. Unmounting it destroys the device synchronously in the effect " +
        "cleanup (like three's WebGPURenderer.dispose()); the native view is dropped one frame later and detaches " +
        "a surface whose swapchain still references the dead device. Expected: a device-lost log line and no crash."
      }
      controls={
        <View class="gap-2">
          <ActionButton
            testID="destroy-toggle"
            label={mounted() ? "Unmount canvas" : "Remount canvas"}
            onPress={() => mounted.update((m) => !m)}
          />
          <If when={!mounted()}>
            <SettingRow
              testID="destroy-surfaceview"
              label="SurfaceView / TextureView"
              detail="Android's backing view of the next mount"
              checked={surfaceView()}
              onChange={(v) => surfaceView.set(v)}
            />
            <SettingRow
              testID="destroy-unconfigure"
              label="context.unconfigure() before device.destroy()"
              detail="Still crashes on a broken build"
              checked={unconfigureFirst()}
              onChange={(v) => unconfigureFirst.set(v)}
            />
          </If>
        </View>
      }
      canvas={
        mounted() ? (
          <RawScene surfaceView={surfaceView()} unconfigureFirst={unconfigureFirst()} append={append} />
        ) : (
          <View style={{ flex: 1 }} />
        )
      }
      log={log()}
    />
  );
};
