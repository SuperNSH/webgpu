// <Canvas>: a view the GPU draws into, react-native-webgpu's Canvas
// (src/Canvas.tsx) on NSH (area: surfaces-apple; the other platforms'
// views are surfaces-other's, behind the same native component).
//
// As in the reference: the canvas allocates a context id once, renders an
// outer <View> with the app's props around the native view
// `webgpu.canvas` (flex: 1) that gets the id, and, once mounted (the
// view's first layout), hands its ref `getContextId`, `getNativeSurface`
// and `getContext("webgpu")`, which makes a context whose canvas is the
// view's layout size in points (set `canvas.width = canvas.clientWidth *
// pixelRatio()` for a sharp image). On unmount the ref gets `null`, and
// `getContext()` on the old ref throws, as the reference's does.
// The native view attaches its surface under the id when it is made; a
// context renders offscreen until then and shows its last frame when it
// does. Unmounting retires the canvas (`destroyContext`), not its
// contexts: as in the reference, a context object keeps its configuration
// and renders offscreen once the view went, and its native state goes with
// the last context object (collected) or the view, whichever is later.
//
// The seam for the other platforms (surfaces-other): every platform
// registers `CANVAS_COMPONENT` and reads the props `nativeCanvasProps`
// gives (the `android*` ones on Android, the `apple*` ones on Apple), as
// the reference's `androidNativeProps` / `appleNativeProps` resolve them.
// The web is the one branch: as the reference's web entry, the browser's
// own WebGPU draws into a `<canvas>` in the view (`./surface/web`), with
// the browser's context; no native module is called there.

import {
  appInfo,
  cleanup,
  effect,
  type JSX,
  nativeComponent,
  platform,
  signal,
  split,
  untracked,
  View,
  type ViewProps,
} from "nsh";
import { forgetContext, makeCanvasContext, nativeSurface, setMountedColorSpace } from "./api/GPUCanvasContext";
import { SURFACE_VERSION } from "./internal/canvasState";
import { call, WgpuCanvasContext } from "./internal/native";
import { allocateContextIdWeb, getNativeSurfaceWeb, isWeb, makeWebGPUCanvasContextWeb, WebCanvasView } from "./surface/web";
import type { CanvasRef, NativeCanvas, RNCanvasContext } from "./types";

export type { CanvasRef, NativeCanvas, RNCanvasContext } from "./types";

/** The name every platform registers its canvas view under. */
export const CANVAS_COMPONENT = "webgpu.canvas";

/** The Android view a canvas draws into, see {@link AndroidCanvasProps}. */
export type AndroidBackingView = "TextureView" | "SurfaceView" | "HardwareBufferView";

/** The options of a canvas backed by a `TextureView`, the default. */
export interface AndroidTextureViewProps {
  /** A `TextureView`: composited like any other view. The default. */
  backingView?: "TextureView";
  /** The canvas covers every pixel of its bounds. Defaults to false. */
  opaque?: boolean;
}

/** The options of a canvas backed by a `SurfaceView`. */
export interface AndroidSurfaceViewProps {
  /** A `SurfaceView`: its own compositor layer, the fastest path. */
  backingView: "SurfaceView";
  /** The canvas covers every pixel of its bounds. Defaults to true. */
  opaque?: boolean;
  /** Composite above every view of the window. Defaults to false. */
  zOrderOnTop?: boolean;
  /** Composite above the other `SurfaceView`s. Defaults to false. */
  zOrderMediaOverlay?: boolean;
}

/** The options of a canvas backed by a `HardwareBufferView`. */
export interface AndroidHardwareBufferViewProps {
  /** A plain view drawing each frame's `AHardwareBuffer` (Android 10+). */
  backingView: "HardwareBufferView";
}

/** Android-only rendering options. Ignored on iOS, macOS and the desktops. */
export type AndroidCanvasProps = AndroidTextureViewProps | AndroidSurfaceViewProps | AndroidHardwareBufferViewProps;

/** `<Canvas>`'s props: a view's, the colorspace of its values, and the ref that gives the context. */
export interface CanvasProps extends Omit<ViewProps, "ref" | "android"> {
  /** Android-only rendering options, see {@link AndroidCanvasProps}. Ignored on Apple platforms. */
  android?: AndroidCanvasProps;
  /**
   * The primaries the canvas's values are in, as `colorSpace` in
   * `context.configure()`: `"srgb"` (the default) or `"display-p3"`. On iOS
   * and macOS the layer is tagged with it when the canvas mounts; a change
   * on a mounted canvas is ignored, with a warning.
   */
  colorSpace?: PredefinedColorSpace;
  /** What happens to values above 1.0 on an `rgba16float` canvas; read when the canvas mounts, like `colorSpace`. */
  toneMapping?: GPUCanvasToneMapping;
  /**
   * react-native-wgpu's former prop (a `TextureView` on Android, which can
   * be transparent). Kept for code written for it: on Android it asks for a
   * `TextureView`; elsewhere transparency follows `alphaMode` in
   * `configure()`.
   */
  transparent?: boolean;
  /**
   * Receives the canvas's `CanvasRef` once it is mounted, and `null` when it
   * unmounts: a callback, or an object whose `current` is set.
   */
  ref?: CanvasRefProp;
}

/**
 * The native canvas view's props, as react-native-webgpu's
 * `WebGPUViewNativeComponent` exports them: a view's props, the context id
 * and the resolved `android` / `colorSpace` / `toneMapping` props.
 */
export interface NativeProps extends Omit<ViewProps, "ref"> {
  /** The canvas's context id. */
  contextId: number;
  /** The `android` prop's backing view, resolved (see `nativeCanvasProps`). */
  androidBackingView?: AndroidBackingView;
  /** Whether the Android view composites as opaque. */
  androidOpaque?: boolean;
  /** A SurfaceView above every view of the window. */
  androidZOrderOnTop?: boolean;
  /** A SurfaceView above the other SurfaceViews. */
  androidZOrderMediaOverlay?: boolean;
  /** The layer is tagged Display P3 (iOS, macOS), read when the canvas mounts. */
  appleDisplayP3?: boolean;
  /** The layer shows extended dynamic range (iOS, macOS), read when the canvas mounts. */
  appleExtendedDynamicRange?: boolean;
}

/** The props of the native view (`WebGPUViewNativeComponent` in the reference). */
interface NativeCanvasProps {
  contextId: number;
  androidBackingView: AndroidBackingView;
  androidOpaque: boolean;
  androidZOrderOnTop: boolean;
  androidZOrderMediaOverlay: boolean;
  appleDisplayP3: boolean;
  appleExtendedDynamicRange: boolean;
}

const WebGPUView = nativeComponent<NativeCanvasProps, {}>(CANVAS_COMPONENT, {
  props: [
    "appleDisplayP3",
    "appleExtendedDynamicRange",
    "androidBackingView",
    "androidOpaque",
    "androidZOrderOnTop",
    "androidZOrderMediaOverlay",
    "contextId",
  ],
});

type AndroidOptions = Partial<{
  backingView: AndroidBackingView;
  opaque: boolean;
  zOrderOnTop: boolean;
  zOrderMediaOverlay: boolean;
}>;

const options = (android?: AndroidCanvasProps): AndroidOptions => (android ?? {}) as AndroidOptions;

const resolveBackingView = (android?: AndroidCanvasProps): AndroidBackingView => {
  const { backingView } = options(android);
  return backingView === "SurfaceView" || backingView === "HardwareBufferView" ? backingView : "TextureView";
};

const resolveOpaque = (android?: AndroidCanvasProps): boolean => {
  const { opaque } = options(android);
  switch (resolveBackingView(android)) {
    case "SurfaceView":
      return opaque !== false;
    case "TextureView":
      return opaque === true;
    case "HardwareBufferView":
      return false;
  }
};

/**
 * The native view's props for a canvas mounted with these props, as the
 * reference's `androidNativeProps` and `appleNativeProps`.
 */
export function nativeCanvasProps(
  contextId: number,
  props: { android?: AndroidCanvasProps; colorSpace?: PredefinedColorSpace; toneMapping?: GPUCanvasToneMapping },
): NativeCanvasProps {
  const backingView = resolveBackingView(props.android);
  const { zOrderOnTop, zOrderMediaOverlay } = options(props.android);
  const surfaceView = backingView === "SurfaceView";
  return {
    contextId,
    androidBackingView: backingView,
    androidOpaque: resolveOpaque(props.android),
    androidZOrderOnTop: surfaceView && !!zOrderOnTop,
    androidZOrderMediaOverlay: surfaceView && !!zOrderMediaOverlay,
    appleDisplayP3: props.colorSpace === "display-p3",
    appleExtendedDynamicRange: props.toneMapping?.mode === "extended",
  };
}

const warnIgnoredChange = (prop: string, why: string) =>
  console.warn(
    `Ignoring a change of ${prop} on a mounted canvas: ${why} Remount the canvas (give it a new key) to change it.`,
  );

/** The reference's `__DEV__`: a development build (`nsh run`, `nsh test`, `nsh build` without `--release`). */
const isDev = (): boolean => {
  try {
    return appInfo().development;
  } catch {
    return false;
  }
};

/** Android's API level (`Platform.Version` in the reference); 0 when unknown. */
const androidApiLevel = (): number => Number(platform.version) || 0;

/**
 * What `<Canvas ref>` takes: a variable (`ref={canvas}`, which NSH compiles
 * to an assignment), a function, or an object whose `current` is set. Each
 * gets the canvas once it is mounted and `null` when it unmounts.
 */
export type CanvasRefProp = CanvasRef | null | ((canvas: CanvasRef | null) => void) | { current: CanvasRef | null };

/** Hands `canvas` (or `null`) to the canvas's `ref` prop. */
const setRef = (ref: CanvasRefProp | undefined, canvas: CanvasRef | null, own: CanvasRef) => {
  if (typeof ref === "function") ref(canvas);
  else if (ref && "current" in ref && (canvas !== null || ref.current === own)) ref.current = canvas;
};

/** The reference's mount-time and change warnings, in development only. */
function useWarnings(own: {
  android?: AndroidCanvasProps;
  colorSpace?: PredefinedColorSpace;
  toneMapping?: GPUCanvasToneMapping;
}, contextId: number, mount: NativeCanvasProps, mountAndroid: AndroidCanvasProps | undefined): void {
  if (!isDev()) return;
  const os = platform.os;
  if (os === "android" && resolveBackingView(mountAndroid) === "HardwareBufferView" && androidApiLevel() < 29) {
    console.warn("android.backingView: HardwareBufferView needs Android 10, the canvas uses a TextureView.");
  }
  let prevAndroid = mountAndroid;
  let prev = mount;
  effect(() => {
    const android = own.android;
    const now = nativeCanvasProps(contextId, { android, colorSpace: own.colorSpace, toneMapping: own.toneMapping });
    untracked(() => {
      if (os === "ios" || os === "macos") {
        const why = "the layer is tagged with it when the canvas mounts.";
        if (now.appleDisplayP3 !== prev.appleDisplayP3) warnIgnoredChange("colorSpace", why);
        if (now.appleExtendedDynamicRange !== prev.appleExtendedDynamicRange) warnIgnoredChange("toneMapping", why);
      }
      if (os === "android") {
        const why = "the backing view is chosen when the canvas mounts.";
        if (resolveBackingView(prevAndroid) !== resolveBackingView(android)) warnIgnoredChange("android.backingView", why);
        // Only a SurfaceView has a z-order of its own, which Android 11 and
        // later changes in place.
        const zOrderAtMount = resolveBackingView(mountAndroid) === "SurfaceView" && androidApiLevel() < 30;
        const was = options(prevAndroid);
        const is = options(android);
        if (zOrderAtMount && !!was.zOrderOnTop !== !!is.zOrderOnTop) warnIgnoredChange("android.zOrderOnTop", why);
        if (zOrderAtMount && !!was.zOrderMediaOverlay !== !!is.zOrderMediaOverlay) {
          warnIgnoredChange("android.zOrderMediaOverlay", why);
        }
      }
      prev = now;
      prevAndroid = android;
    });
  });
}

/**
 * A view WebGPU draws into: `ref.getContext("webgpu")`. As in the
 * reference, the ref is set once the canvas is mounted (here: its view's
 * first layout, so `getContext()` sizes the canvas to a real layout), and
 * set to `null` when it unmounts; `getContext()` on an unmounted canvas
 * throws "[WebGPU] Cannot get context before mount". `children` are
 * accepted and not rendered, as the reference's own native view replaces
 * them.
 */
export function Canvas(props: CanvasProps): JSX.Element {
  if (isWeb()) return WebCanvas(props);
  // Native hands out the id: unique for the life of the process.
  const contextId = call(() => WgpuCanvasContext.allocateContextId());
  const [own, rest] = split(props, ["ref", "android", "colorSpace", "toneMapping", "transparent", "children"]);
  const mountAndroid = untracked(
    () => own.android ?? (own.transparent ? ({ backingView: "TextureView" } as AndroidCanvasProps) : undefined),
  );
  // Read once: the view is made from these when the canvas mounts.
  const mount = untracked(() =>
    nativeCanvasProps(contextId, {
      android: mountAndroid,
      colorSpace: own.colorSpace,
      toneMapping: own.toneMapping,
    }),
  );
  setMountedColorSpace(contextId, {
    colorSpace: mount.appleDisplayP3 ? "display-p3" : "srgb",
    toneMappingMode: mount.appleExtendedDynamicRange ? "extended" : "standard",
  });
  useWarnings(own, contextId, mount, mountAndroid);

  // The view's layout size in points: what getContext() sizes the canvas to.
  let layout = { width: 0, height: 0 };
  let mounted = false;
  let gone = false;
  // Bumped when what getNativeSurface() reads may have changed (a layout,
  // the surface attaching): what useSurface() follows.
  const version = signal(0);
  const size = () => {
    if (layout.width > 0 || layout.height > 0) return layout;
    const view = nativeSurface(contextId);
    return { width: view.width, height: view.height };
  };

  const canvasRef: CanvasRef = {
    getContextId: () => contextId,
    getNativeSurface: (): NativeCanvas => {
      const view = nativeSurface(contextId);
      // Before the view reports its size (or where no platform view draws,
      // headless), the layout's: the same size in points.
      const { width, height } = view.width > 0 || view.height > 0 ? view : layout;
      return {
        surface: BigInt(view.surface),
        width,
        height,
        clientWidth: width,
        clientHeight: height,
        // No-ops, so web renderers (three.js) take it as an HTMLCanvasElement.
        addEventListener: () => {},
        removeEventListener: () => {},
        dispatchEvent: () => {},
        setPointerCapture: () => {},
        releasePointerCapture: () => {},
      } as NativeCanvas;
    },
    getContext(contextName: "webgpu"): RNCanvasContext | null {
      if (contextName !== "webgpu") {
        throw new Error(`[WebGPU] Unsupported context: ${contextName}`);
      }
      if (!mounted) {
        throw new Error("[WebGPU] Cannot get context before mount");
      }
      const { width, height } = size();
      return makeCanvasContext(contextId, width, height);
    },
  };
  Object.defineProperty(canvasRef, SURFACE_VERSION, { value: () => version(), enumerable: false });

  // The view attaches its surface on the UI thread, which may land after the
  // first layout: watch a few frames for it, so useSurface() sees it.
  let watch: number | null = null;
  const watchAttach = (frames: number) => {
    if (gone || frames <= 0 || typeof globalThis.requestAnimationFrame !== "function") return;
    watch = globalThis.requestAnimationFrame(() => {
      watch = null;
      if (gone) return;
      if (nativeSurface(contextId).surface !== "0") version.update((v) => v + 1);
      else watchAttach(frames - 1);
    });
  };

  cleanup(() => {
    gone = true;
    mounted = false;
    if (watch !== null) globalThis.cancelAnimationFrame(watch);
    setRef(untracked(() => own.ref), null, canvasRef);
    forgetContext(contextId);
    call(() => WgpuCanvasContext.destroyContext(contextId));
  });

  return (
    <View {...rest}>
      <WebGPUView
        style={{ flex: 1 }}
        onLayout={(e) => {
          layout = { width: e.bounds.size.width, height: e.bounds.size.height };
          if (gone) return;
          if (!mounted) {
            mounted = true;
            setRef(untracked(() => own.ref), canvasRef, canvasRef);
            if (nativeSurface(contextId).surface === "0") watchAttach(120);
          }
          version.update((v) => v + 1);
        }}
        {...mount}
      />
    </View>
  );
}

/**
 * `<Canvas>` on the web, as the reference's web entry has it: the id from
 * the polyfill's counter, a `<canvas>` in the view, the browser's context
 * sized to the view's layout (CSS pixels × `devicePixelRatio`), and
 * `getNativeSurface()` with the DOM canvas's own event methods. The ref is
 * set after the first layout and cleared on unmount, as natively; nothing
 * to destroy (the `<canvas>` goes with its view).
 */
function WebCanvas(props: CanvasProps): JSX.Element {
  const contextId = allocateContextIdWeb();
  const [own, rest] = split(props, ["ref", "android", "colorSpace", "toneMapping", "transparent", "children"]);
  let mounted = false;
  let gone = false;
  const version = signal(0);
  const canvasRef: CanvasRef = {
    getContextId: () => contextId,
    getNativeSurface: () => getNativeSurfaceWeb(contextId) as unknown as NativeCanvas,
    getContext(contextName: "webgpu"): RNCanvasContext | null {
      if (contextName !== "webgpu") {
        throw new Error(`[WebGPU] Unsupported context: ${contextName}`);
      }
      if (!mounted) {
        throw new Error("[WebGPU] Cannot get context before mount");
      }
      const { width, height } = getNativeSurfaceWeb(contextId);
      return makeWebGPUCanvasContextWeb(contextId, width, height) as unknown as RNCanvasContext | null;
    },
  };
  Object.defineProperty(canvasRef, SURFACE_VERSION, { value: () => version(), enumerable: false });
  cleanup(() => {
    gone = true;
    mounted = false;
    setRef(untracked(() => own.ref), null, canvasRef);
  });
  return (
    <View {...rest}>
      <View
        style={{ flex: 1 }}
        onLayout={() => {
          if (gone) return;
          if (!mounted) {
            mounted = true;
            setRef(untracked(() => own.ref), canvasRef, canvasRef);
          }
          version.update((v) => v + 1);
        }}
      >
        <WebCanvasView contextId={contextId} />
      </View>
    </View>
  );
}
