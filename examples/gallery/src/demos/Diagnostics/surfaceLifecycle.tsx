// apps/example/src/Diagnostics/surfaceLifecycle.tsx on NSH: the shared
// plumbing of the surface lifecycle repros (a GPU with its errors logged, a
// trivial animated clear, the on-screen log), and the screens' common
// layout (description, controls, canvas, log) as a component.

import { type JSX, Scroll, signal, Text, View } from "nsh";
import type { RNCanvasContext } from "nsh-webgpu";

// Shared plumbing for the surface lifecycle repros. Each screen renders a
// trivial animated clear so that getCurrentTexture()/submit()/present() run
// every frame; validation errors are surfaced through the uncapturederror
// event so lifecycle holes that do not hard-crash still show up in the log.

export interface DiagnosticGPU {
  device: GPUDevice;
  format: GPUTextureFormat;
}

export const initGPU = async (onLog: (message: string) => void): Promise<DiagnosticGPU> => {
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) {
    throw new Error("No adapter");
  }
  const device = await adapter.requestDevice();
  device.addEventListener("uncapturederror", (event) => {
    const { error } = event as GPUUncapturedErrorEvent;
    onLog(`[uncaptured] ${error.message.split("\n")[0]}`);
  });
  device.lost.then((info) => {
    onLog(`[device lost] ${info.reason}: ${info.message}`);
  });
  return { device, format: navigator.gpu.getPreferredCanvasFormat() };
};

export const drawClearFrame = (device: GPUDevice, context: RNCanvasContext, frame: number, clearValue?: GPUColor) => {
  const texture = context.getCurrentTexture();
  const view = texture.createView();
  const encoder = device.createCommandEncoder();
  const t = frame / 60;
  const pass = encoder.beginRenderPass({
    colorAttachments: [
      {
        view,
        clearValue: clearValue ?? [0.5 + 0.5 * Math.sin(t), 0.5 + 0.5 * Math.sin(t + 2.1), 0.5 + 0.5 * Math.sin(t + 4.2), 1],
        loadOp: "clear",
        storeOp: "store",
      },
    ],
  });
  pass.end();
  device.queue.submit([encoder.finish()]);
  context.present();
};

export const useDiagnosticLog = () => {
  const log = signal<string[]>([]);
  const append = (line: string) => {
    console.log(`[surface-lifecycle] ${line}`);
    log.update((prev) => [...prev.slice(-24), line]);
  };
  return { log, append };
};

/** A diagnostic screen: its description, its controls, its canvas (or what replaces it) and the log. */
export function DiagnosticScreen(props: {
  description: string;
  controls?: JSX.Element;
  canvas: JSX.Element;
  log: string[];
}) {
  return (
    <View class="flex-1">
      <View class="gap-2 p-3">
        <Text class="text-xs text-muted-foreground">{props.description}</Text>
        {props.controls}
      </View>
      <View class="flex-1">{props.canvas}</View>
      <Scroll class="max-h-[200px] bg-muted p-3" testID="diag-log">
        <Each of={props.log}>
          {(line) => (
            <Text class="font-mono text-[10px] text-foreground" testID="diag-line">
              {line()}
            </Text>
          )}
        </Each>
      </Scroll>
    </View>
  );
}
