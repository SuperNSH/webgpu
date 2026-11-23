// react-native-webgpu's hooks (its src/hooks.tsx), for NSH (area:
// react-parity). NSH has no React: a component runs once and only the
// bindings that read a changed signal run again. So each hook keeps the
// reference's name, arguments and return shape, and what React would
// re-render with is a signal underneath: the returned objects' members are
// getters reading signals, so a JSX binding or an `effect` that reads
// `state.device` runs again once the device is there. Destructuring
// (`const { device } = useDevice()`) takes a snapshot, as reading any
// signal in a component body does. docs/parity/react-parity.md maps each.

import { computed, signal, untracked } from "nsh";
import { surfaceVersion } from "./internal/canvasState";
import type { CanvasRef, NativeCanvas } from "./types";

/**
 * Warns on the console when `adapter` is a software fallback
 * (`adapter.info.isFallbackAdapter`), as react-native-webgpu does.
 */
export const warnIfNotHardwareAccelerated = (adapter: GPUAdapter): void => {
  if (adapter.info && adapter.info.isFallbackAdapter) {
    console.warn(
      "GPUAdapter is not hardware accelerated. This is common on Android emulators. Rendering will be slow. Some features may be unavailable.",
    );
  }
};

/**
 * React's `RefObject<CanvasRef>` for NSH: pass it as `<Canvas ref={ref}>`
 * (it is the callback NSH calls with the canvas) and read `ref.current`.
 * `current` is a signal: a binding or effect reading it runs again when the
 * canvas arrives.
 */
export type CanvasRefObject = ((canvas: CanvasRef | null) => void) & {
  /** The canvas, `null` before `<Canvas>` set it. */
  current: CanvasRef | null;
};

/** A `{ ref, surface }` pair: `surface` is the canvas's native surface once `ref` holds a canvas. */
export interface SurfaceState {
  /** Pass it to `<Canvas ref>`. */
  ref: CanvasRefObject;
  /** The canvas's native surface, `null` until the canvas mounted. */
  readonly surface: NativeCanvas | null;
}

/** An adapter and a device, both `null` until the request finished. */
export interface DeviceState {
  /** The adapter, `null` until ready. */
  readonly adapter: GPUAdapter | null;
  /** The device, `null` until ready. */
  readonly device: GPUDevice | null;
}

/** A ref for `<Canvas>`: `useRef<CanvasRef>(null)` in the reference. */
export const useCanvasRef = (): CanvasRefObject => {
  const current = signal<CanvasRef | null>(null);
  const ref = ((canvas: CanvasRef | null) => current.set(canvas)) as CanvasRefObject;
  Object.defineProperty(ref, "current", {
    get: () => current(),
    set: (canvas: CanvasRef | null) => current.set(canvas),
    enumerable: true,
  });
  return ref;
};

/**
 * `{ ref, surface }`: pass `ref` to `<Canvas>`; `surface` becomes the
 * canvas's `getNativeSurface()` once the canvas mounted (the reference
 * reads it in a layout effect after mount), is read again when the canvas
 * lays out anew or its surface attaches, and is `null` again once the
 * canvas unmounted.
 */
export const useSurface = (): SurfaceState => {
  const ref = useCanvasRef();
  const surface = computed(() => {
    const canvas = ref.current;
    if (!canvas) return null;
    surfaceVersion(canvas);
    return untracked(() => canvas.getNativeSurface());
  });
  return {
    ref,
    get surface() {
      return surface();
    },
  };
};

/**
 * Requests an adapter, then a device, once. Returns `{ adapter, device }`,
 * both `null` until ready (signals: bindings reading them update). Throws
 * as the reference does, in the request's promise: `No appropriate
 * GPUAdapter found.`, `No appropriate GPUDevice found.`
 */
export const useDevice = (adapterOptions?: GPURequestAdapterOptions, deviceDescriptor?: GPUDeviceDescriptor): DeviceState => {
  const state = signal<{ adapter: GPUAdapter; device: GPUDevice } | null>(null);
  void (async () => {
    const adapter = await navigator.gpu.requestAdapter(adapterOptions);
    if (!adapter) {
      throw new Error("No appropriate GPUAdapter found.");
    }
    warnIfNotHardwareAccelerated(adapter);
    const device = await adapter.requestDevice(deviceDescriptor);
    if (!device) {
      throw new Error("No appropriate GPUDevice found.");
    }
    state.set({ adapter, device });
  })();
  return {
    get adapter() {
      return state()?.adapter ?? null;
    },
    get device() {
      return state()?.device ?? null;
    },
  };
};
