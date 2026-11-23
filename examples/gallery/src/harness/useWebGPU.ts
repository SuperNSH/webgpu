// The reference's components/useWebGPU.ts on NSH: a scene gets a device, a
// configured canvas context (premultiplied, the preferred format, the
// drawing buffer at the view's size times the pixel ratio) and may return a
// function rendering one frame, which then runs on requestAnimationFrame
// followed by `context.present()`.
//
// NSH differences: a component runs once, so the hook starts when the
// canvas hands over its ref (the returned callback; <Canvas> sets it once
// mounted and laid out, as the reference's ref is set before its effects)
// instead of a React effect; the device comes from
// `requestAdapter()`/`requestDevice()` (the reference's `useDevice()` does
// the same) and is destroyed when the page goes. The page's probe
// (harness/probe.ts) watches the device and captures one frame.

import { cleanup, pixelRatio } from "nsh";
import {
  type CanvasRef,
  cancelAnimationFrame,
  type NativeCanvas,
  type RNCanvasContext,
  requestAnimationFrame,
} from "nsh-webgpu";
import { type Probe, ProbeContext } from "./probe";

export interface SceneProps {
  context: RNCanvasContext;
  device: GPUDevice;
  gpu: GPU;
  presentationFormat: GPUTextureFormat;
  canvas: NativeCanvas;
}

export type RenderScene = (timestamp: number) => void;
export type Scene = (props: SceneProps) => RenderScene | void | Promise<RenderScene | void>;

/** Resolves once the canvas has a layout size, with its context. */
export async function contextAfterLayout(ref: CanvasRef, isGone: () => boolean): Promise<RNCanvasContext | null> {
  for (let i = 0; i < 600 && !isGone(); i++) {
    if (ref.getNativeSurface().clientWidth > 0 || i > 0) {
      const context = ref.getContext("webgpu");
      if (context && context.canvas.clientWidth > 0 && context.canvas.clientHeight > 0) return context;
    }
    await new Promise((resolve) => requestAnimationFrame(resolve));
  }
  return null;
}

/** Requests a device, as `useDevice()` does. */
export async function requestDevice(descriptor?: GPUDeviceDescriptor): Promise<GPUDevice> {
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("No appropriate GPUAdapter found.");
  const device = await adapter.requestDevice(descriptor);
  if (!device) throw new Error("No appropriate GPUDevice found.");
  return device;
}

/** The probe of the page this component is on. */
export const useProbe = (): Probe => ProbeContext();

export const useWebGPU = (
  scene: Scene,
  deviceDescriptor?: GPUDeviceDescriptor,
): ((ref: CanvasRef | null) => void) => {
  const probe = useProbe();
  let gone = false;
  let animationFrameId: number | null = null;
  let device: GPUDevice | undefined;
  cleanup(() => {
    gone = true;
    if (animationFrameId !== null) cancelAnimationFrame(animationFrameId);
    device?.destroy();
  });

  const start = async (ref: CanvasRef) => {
    device = probe.watch(await requestDevice(deviceDescriptor));
    const context = await contextAfterLayout(ref, () => gone);
    if (!context || gone) return;
    probe.context(context);
    const canvas = context.canvas;
    const presentationFormat = navigator.gpu.getPreferredCanvasFormat();
    canvas.width = canvas.clientWidth * pixelRatio();
    canvas.height = canvas.clientHeight * pixelRatio();
    context.configure({ device, format: presentationFormat, alphaMode: "premultiplied" });

    const renderScene = await scene({ context, device, gpu: navigator.gpu, presentationFormat, canvas });
    if (gone) return;
    if (typeof renderScene !== "function") return;
    const gpuDevice = device;
    const render = () => {
      if (gone) return;
      try {
        const timestamp = Date.now();
        probe.run(() => renderScene(timestamp));
        probe.frame(gpuDevice, context);
        context.present();
      } catch (error) {
        probe.fail(error);
        return;
      }
      // In snapshot mode the compared frame stays on screen.
      if (probe.snapshotDone) return;
      animationFrameId = requestAnimationFrame(render);
    };
    animationFrameId = requestAnimationFrame(render);
  };

  // The canvas once mounted, then null when it unmounts (the cleanup above
  // stops the scene).
  return (ref: CanvasRef | null) => {
    if (ref) start(ref).catch((error: unknown) => probe.fail(error));
  };
};

/**
 * For the demos that drive the canvas themselves (the reference's
 * `useRef<CanvasRef>` + `useEffect`): runs `start` once the canvas element
 * exists, with a function telling whether the page is gone. Failures go to
 * the page's probe.
 */
export const useCanvas = (start: (ref: CanvasRef, isGone: () => boolean) => Promise<void>) => {
  const probe = useProbe();
  let gone = false;
  cleanup(() => {
    gone = true;
  });
  return (ref: CanvasRef | null) => {
    if (ref) start(ref, () => gone).catch((error: unknown) => probe.fail(error));
  };
};
