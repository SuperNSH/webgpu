// <NativeBufferView> and useNativeBufferSize (react-native-webgpu's
// src/NativeBufferView.tsx), area: react-parity. The view shows an
// IOSurface / AHardwareBuffer with no swap chain: nsh-webgpu has no such
// native view and no way to make those buffers (createNativeBuffer), so it
// throws ERR_WEBGPU_UNSUPPORTED (docs/parity/react-parity.md).
// useNativeBufferSize is plain logic and works: React's useState is a
// signal (`size` is a getter reading it), the timer is `setTimeout`, and
// the unmount cleanup is NSH's `cleanup()`.

import { cleanup, type JSX, signal, type ViewProps } from "nsh";
import { unsupportedError } from "./internal/native";
import type { GPUSharedFenceState, NativeBuffer } from "./types";

/**
 * How a view fits a buffer of another size: `stretch` (the default),
 * `contain`, `cover`, or `topLeft` (not scaled, cropped).
 */
export type NativeBufferResizeMode = "stretch" | "contain" | "cover" | "topLeft";

/** A size in physical pixels. */
export interface NativeBufferPixelSize {
  width: number;
  height: number;
}

/** What a `<NativeBufferView ref>` holds. */
export interface NativeBufferViewRef {
  /** The id for `RNWebGPU.presentNativeBuffer(viewId, frame, done)`. */
  readonly viewId: number;
  /** Shows `frame` once the fences of `done` are signalled. */
  present(frame: NativeBuffer, done?: { fences: GPUSharedFenceState[] } | null): void;
}

/** `<NativeBufferView>`'s props. */
export interface NativeBufferViewProps extends Omit<ViewProps, "ref"> {
  /** See `NativeBufferResizeMode`; `"stretch"` by default. */
  resizeMode?: NativeBufferResizeMode;
  /** The view's size in physical pixels, on mount and on every change. */
  onPixelSizeChange?: (size: NativeBufferPixelSize) => void;
  /** Receives the view's `NativeBufferViewRef`. */
  ref?: (view: NativeBufferViewRef) => void;
}

/** Displays a native buffer with no swap chain: unsupported in nsh-webgpu. */
export const NativeBufferView = (props: NativeBufferViewProps): JSX.Element => {
  void props;
  throw unsupportedError(
    "NativeBufferView is unavailable: nsh-webgpu has no native view that displays an IOSurface or AHardwareBuffer, and no way to make one (createNativeBuffer)",
  );
};

/** What `useNativeBufferSize()` returns. */
export interface NativeBufferSizeState {
  /** The settled size, `null` before the first report. A signal: bindings reading it update. */
  readonly size: NativeBufferPixelSize | null;
  /** Pass it as `<NativeBufferView onPixelSizeChange>`. */
  onPixelSizeChange: (next: NativeBufferPixelSize) => void;
}

/**
 * Tracks a view's pixel size, settling resizes: the first size is reported
 * at once, later ones once the size stayed the same for `debounceMs`
 * (`<= 0`: at once). The timer stops with the component.
 */
export const useNativeBufferSize = (debounceMs = 150): NativeBufferSizeState => {
  const size = signal<NativeBufferPixelSize | null>(null);
  let current: NativeBufferPixelSize | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;

  const onPixelSizeChange = (next: NativeBufferPixelSize) => {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    const settle = () => {
      timer = null;
      const previous = current;
      if (previous === null || previous.width !== next.width || previous.height !== next.height) {
        current = next;
        size.set(next);
      }
    };
    if (current === null || debounceMs <= 0) {
      settle();
    } else {
      timer = setTimeout(settle, debounceMs);
    }
  };

  cleanup(() => {
    if (timer !== null) {
      clearTimeout(timer);
    }
  });

  return {
    get size() {
      return size();
    },
    onPixelSizeChange,
  };
};
