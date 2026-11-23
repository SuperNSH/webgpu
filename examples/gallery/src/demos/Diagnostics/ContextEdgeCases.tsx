// apps/example/src/Diagnostics/ContextEdgeCases.tsx on NSH.

import { cleanup, View } from "nsh";
import type { CanvasRef } from "nsh-webgpu";
import { Canvas } from "nsh-webgpu";
import { ActionButton } from "../../ui";

import { DiagnosticScreen, drawClearFrame, initGPU, useDiagnosticLog } from "./surfaceLifecycle";

// Repros for GPUCanvasContext entry points the WebGPU spec expects to fail
// gracefully but that crash or silently misbehave in the native
// implementation:
//
// 1. getCurrentTexture() before configure(): SurfaceInfo has no device yet.
// 2. configure() on a 0x0 canvas: Dawn refuses zero-sized textures, so
//    getCurrentTexture() wraps a null texture and createView() dereferences
//    it. A canvas can legitimately be measured at 0x0 for a frame (collapsed
//    layout, display: none equivalents), so this is reachable from ordinary
//    app code.
// 3. unconfigure(): after unconfigure() the canvas should behave as if it
//    was never configured.
//
// Each button is an independent repro.
export const ContextEdgeCases = () => {
  const ref: { current: CanvasRef | null } = { current: null };
  const { log, append } = useDiagnosticLog();
  const devices: GPUDevice[] = [];
  cleanup(() => devices.forEach((device) => device.destroy()));
  const gpu = async () => {
    const result = await initGPU(append);
    devices.push(result.device);
    return result;
  };

  const getCurrentTextureUnconfigured = () => {
    try {
      const ctx = ref.current!.getContext("webgpu")!;
      append("getCurrentTexture() before configure()...");
      const texture = ctx.getCurrentTexture();
      append(`returned a ${texture.width}x${texture.height} texture (spec: should throw)`);
    } catch (e) {
      append(`threw (correct per spec): ${e}`);
    }
  };

  const zeroSizedCanvas = async () => {
    try {
      const { device, format } = await gpu();
      const ctx = ref.current!.getContext("webgpu")!;
      const canvas = ctx.canvas;
      canvas.width = 0;
      canvas.height = 0;
      append("configure() with canvas.width = canvas.height = 0...");
      ctx.configure({ device, format, alphaMode: "opaque" });
      const texture = ctx.getCurrentTexture();
      append(`getCurrentTexture() -> ${texture.width}x${texture.height}`);
      texture.createView();
      append("createView() survived");
    } catch (e) {
      append(`threw: ${e}`);
    }
  };

  const unconfigureStub = async () => {
    try {
      const { device, format } = await gpu();
      const ctx = ref.current!.getContext("webgpu")!;
      ctx.configure({ device, format, alphaMode: "opaque" });
      drawClearFrame(device, ctx, 0);
      append("rendered one frame, calling unconfigure()...");
      ctx.unconfigure();
      const texture = ctx.getCurrentTexture();
      append(
        `getCurrentTexture() after unconfigure() -> ${texture.width}x${texture.height} texture (spec: context should be unconfigured)`,
      );
    } catch (e) {
      append(`threw (correct per spec): ${e}`);
    }
  };

  return (
    <DiagnosticScreen
      description={
        "Each button exercises a context entry point that must fail gracefully per the WebGPU spec. On a broken " +
        "build the first two crash the app."
      }
      controls={
        <View class="gap-2">
          <ActionButton
            testID="edge-unconfigured"
            label="getCurrentTexture() before configure()"
            onPress={getCurrentTextureUnconfigured}
          />
          <ActionButton testID="edge-zero" label="configure() a 0x0 canvas" onPress={() => void zeroSizedCanvas()} />
          <ActionButton
            testID="edge-unconfigure"
            label="unconfigure() then getCurrentTexture()"
            onPress={() => void unconfigureStub()}
          />
        </View>
      }
      canvas={<Canvas ref={(canvas) => (ref.current = canvas)} style={{ flex: 1 }} />}
      log={log()}
    />
  );
};
