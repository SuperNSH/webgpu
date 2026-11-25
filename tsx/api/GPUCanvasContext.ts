// GPUCanvasContext (area: surfaces-apple): what `canvas.getContext("webgpu")`
// gives, react-native-webgpu's `RNCanvasContext` (`present()` included).
//
// The native half (src/api/canvas_context.rs) keeps the swap chain by
// context id, as the reference's SurfaceInfo: every context object of one
// <Canvas> shares it, and it renders offscreen until the view's surface
// attaches (then shows the last presented frame at once). This file keeps
// what only TSX can hold: the configuration with its device object, the
// current texture's wrapper (the same object until the frame ends), and the
// canvas object (`context.canvas`, `getNativeSurface()`).

import { call, WgpuCanvasContext } from "../internal/native";
import { INTERNAL, handleOf, internal } from "../internal/object";
import type { SpecGPUCanvasContext } from "../types/spec";
import type { GPUDevice } from "./GPUDevice";
import { type GPUTexture, wrapTexture } from "./GPUTexture";

/** `GPUTextureUsage.RENDER_ATTACHMENT`: a canvas's default usage. */
const RENDER_ATTACHMENT = 0x10;

/** The colorspace a mounted <Canvas> tagged its layer with (Apple). */
export interface MountedColorSpace {
  colorSpace: PredefinedColorSpace;
  toneMappingMode: GPUCanvasToneMappingMode;
}

/** What every context object of a canvas shares. */
interface Shared {
  configuration: GPUCanvasConfigurationOut | null;
  device: GPUDevice | null;
  /** The current frame's texture: its native id and its wrapper. */
  current: { id: number; texture: GPUTexture } | null;
  /** The <Canvas>'s colorSpace / toneMapping props, when one mounted it. */
  mounted: MountedColorSpace | null;
  /** configure() already warned about a colorspace mismatch. */
  warned: boolean;
}

/** The state of each mounted <Canvas>'s contexts, by context id. */
const shared = new Map<number, Shared>();

/**
 * Tells the native side when a context object is collected, so the native
 * state of a canvas that unmounted goes with its last context object (the
 * reference's `SurfaceInfo` is held by its `GPUCanvasContext`s the same way).
 */
const released =
  typeof FinalizationRegistry === "function"
    ? new FinalizationRegistry<number>((contextId) => {
        try {
          call(() => WgpuCanvasContext.release(contextId));
        } catch {
          // Gone already.
        }
      })
    : undefined;

function stateOf(contextId: number): Shared {
  let state = shared.get(contextId);
  if (!state) {
    state = { configuration: null, device: null, current: null, mounted: null, warned: false };
    shared.set(contextId, state);
  }
  return state;
}

/** The <Canvas> of `contextId` mounted with these colorspace props. */
export function setMountedColorSpace(contextId: number, mounted: MountedColorSpace): void {
  stateOf(contextId).mounted = mounted;
}

/**
 * The <Canvas> of `contextId` unmounted: no new context object shares its
 * state. The context objects made before keep theirs and keep working
 * (offscreen once the view went), as in the reference.
 */
export function forgetContext(contextId: number): void {
  shared.delete(contextId);
  const retry = retries.get(contextId);
  if (retry !== undefined) clearTimeout(retry);
  retries.delete(contextId);
}

/** The retries of {@link showWaiting}, by context id. */
const retries = new Map<number, ReturnType<typeof setTimeout>>();

/** How long between looks at whether a window is visible again. */
const RETRY_MS = 100;

/**
 * Shows the last presented frame of `contextId` if its view does not show
 * it yet, and tries again while the window is occluded: a canvas drawn once
 * has no next frame to show instead.
 */
export function showWaiting(contextId: number, waiting = call(() => WgpuCanvasContext.showWaiting(contextId))): void {
  if (!waiting || retries.has(contextId)) return;
  retries.set(
    contextId,
    setTimeout(() => {
      retries.delete(contextId);
      showWaiting(contextId);
    }, RETRY_MS),
  );
}

/** A canvas size as a `<canvas>` keeps it: a whole number of pixels, 0 for what is not a number. */
function pixels(value: number): number {
  const n = Math.trunc(Number(value));
  return n > 0 && Number.isFinite(n) ? n : 0;
}

/**
 * The canvas a context draws into (react-native-webgpu's `Canvas` host
 * object, `NativeCanvas`): `width` / `height` are the drawing buffer's
 * size, set by the app (`canvas.width = canvas.clientWidth * pixelRatio()`);
 * `clientWidth` / `clientHeight` the view's size in points. The event
 * methods are no-ops, so code written for an `HTMLCanvasElement` (three.js)
 * takes it.
 */
export class CanvasElement {
  #width = 0;
  #height = 0;
  /** The view's width in points. */
  clientWidth: number;
  /** The view's height in points. */
  clientHeight: number;
  readonly #contextId: number;

  constructor(token: unknown, contextId: number, width: number, height: number) {
    internal(token);
    this.#contextId = contextId;
    this.width = width;
    this.height = height;
    this.clientWidth = width;
    this.clientHeight = height;
  }

  /** The drawing buffer's width in pixels: an integer, a fraction is truncated (the reference's `int`). */
  get width(): number {
    return this.#width;
  }
  set width(value: number) {
    this.#width = pixels(value);
  }

  /** The drawing buffer's height in pixels, as {@link width}. */
  get height(): number {
    return this.#height;
  }
  set height(value: number) {
    this.#height = pixels(value);
  }

  /** The native surface (the `CAMetalLayer *` on Apple), `0n` before the view attached. */
  get surface(): bigint {
    return BigInt(nativeSurface(this.#contextId).surface);
  }

  /** No-op (DOM compatibility). */
  addEventListener(_type: string, _listener: unknown): void {}
  /** No-op (DOM compatibility). */
  removeEventListener(_type: string, _listener: unknown): void {}
  /** No-op (DOM compatibility). */
  dispatchEvent(_event: unknown): boolean {
    return true;
  }
  /** No-op (DOM compatibility). */
  setPointerCapture(_pointerId: number): void {}
  /** No-op (DOM compatibility). */
  releasePointerCapture(_pointerId: number): void {}

  get [Symbol.toStringTag](): string {
    return "Canvas";
  }
}

/** What the native side knows of the view of `contextId`. */
export function nativeSurface(contextId: number): {
  surface: string;
  width: number;
  height: number;
  scale: number;
} {
  return call(() => WgpuCanvasContext.nativeSurface(contextId)) as {
    surface: string;
    width: number;
    height: number;
    scale: number;
  };
}

/** A canvas's WebGPU context: its swap chain. */
export class GPUCanvasContext implements SpecGPUCanvasContext {
  declare readonly __brand: "GPUCanvasContext";
  readonly #canvas: CanvasElement;
  readonly #contextId: number;
  /** Shared with the canvas's other context objects; outlives the <Canvas>. */
  readonly #state: Shared;

  constructor(token: unknown, canvas: CanvasElement, contextId: number) {
    internal(token);
    this.#canvas = canvas;
    this.#contextId = contextId;
    this.#state = stateOf(contextId);
    call(() => WgpuCanvasContext.make(contextId));
    released?.register(this, contextId);
  }

  /** The canvas (react-native-webgpu's `Canvas`: `width`, `height`, `clientWidth`, `clientHeight`). */
  get canvas(): HTMLCanvasElement & CanvasElement {
    return this.#canvas as HTMLCanvasElement & CanvasElement;
  }

  /** Sets the device, format, usage, view formats, alpha mode and colorspace of the swap chain. */
  configure(configuration: GPUCanvasConfiguration): undefined {
    const device = configuration.device;
    const deviceId = handleOf(device, "GPUDevice");
    const viewFormats = Array.from(configuration.viewFormats ?? []);
    const colorSpace = configuration.colorSpace ?? "srgb";
    const toneMapping = { mode: configuration.toneMapping?.mode ?? "standard" };
    const alphaMode = configuration.alphaMode ?? "opaque";
    const usage = configuration.usage ?? RENDER_ATTACHMENT;
    call(() =>
      WgpuCanvasContext.configure(this.#contextId, {
        device: deviceId,
        format: configuration.format,
        usage,
        viewFormats,
        colorSpace,
        toneMapping,
        alphaMode,
        width: Math.max(1, Math.floor(this.#canvas.width)),
        height: Math.max(1, Math.floor(this.#canvas.height)),
      }),
    );
    const state = this.#state;
    state.device = device as unknown as GPUDevice;
    state.current = null;
    state.configuration = {
      device,
      format: configuration.format,
      usage,
      viewFormats,
      colorSpace,
      toneMapping,
      alphaMode,
    };
    // The layer was tagged from the <Canvas> props when it mounted (Apple):
    // say so once when configure() asks for something else.
    const mounted = state.mounted;
    if (
      mounted &&
      !state.warned &&
      (mounted.colorSpace !== colorSpace || mounted.toneMappingMode !== toneMapping.mode)
    ) {
      state.warned = true;
      console.warn(
        `[WebGPU] configure() asks for colorSpace "${colorSpace}" and toneMapping "${toneMapping.mode}", ` +
          `but the canvas was mounted with colorSpace "${mounted.colorSpace}" and toneMapping ` +
          `"${mounted.toneMappingMode}". On iOS and macOS these come from the colorSpace and toneMapping ` +
          "props of the Canvas, read when it mounts: pass the same values there.",
      );
    }
    return undefined;
  }

  /** Drops the swap chain's configuration. */
  unconfigure(): undefined {
    call(() => WgpuCanvasContext.unconfigure(this.#contextId));
    const state = this.#state;
    state.configuration = null;
    state.device = null;
    state.current = null;
    return undefined;
  }

  /** The configuration, or `null` before configure() and after unconfigure(). */
  getConfiguration(): GPUCanvasConfigurationOut | null {
    const configuration = this.#state.configuration;
    if (!configuration) return null;
    return {
      ...configuration,
      viewFormats: [...configuration.viewFormats],
      toneMapping: { ...configuration.toneMapping },
    };
  }

  /**
   * The texture to draw this frame into, the same object until the frame
   * ends (`present()`, `configure()`, `unconfigure()`, or a new
   * `canvas.width` / `height`). An `InvalidStateError` before `configure()`.
   */
  getCurrentTexture(): GPUTexture {
    const width = Math.max(1, Math.floor(this.#canvas.width));
    const height = Math.max(1, Math.floor(this.#canvas.height));
    const id = call(() => WgpuCanvasContext.getCurrentTexture(this.#contextId, width, height));
    const state = this.#state;
    if (state.current?.id === id) return state.current.texture;
    const configuration = state.configuration;
    const device = state.device;
    if (!configuration || !device) {
      throw new DOMException("getCurrentTexture() called on a canvas context that is not configured", "InvalidStateError");
    }
    const texture = wrapTexture(device, id, {
      size: [width, height, 1],
      format: configuration.format,
      usage: configuration.usage,
      viewFormats: configuration.viewFormats,
      dimension: "2d",
      mipLevelCount: 1,
      sampleCount: 1,
    });
    state.current = { id, texture };
    const view = nativeSurface(this.#contextId);
    if (view.width > 0 && view.height > 0) {
      this.#canvas.clientWidth = view.width;
      this.#canvas.clientHeight = view.height;
    }
    return texture;
  }

  /**
   * Shows the frame (react-native-webgpu's `present()`): call it after
   * `queue.submit()`. A frame drawn before the view's surface attached is
   * shown when it does.
   */
  present(): void {
    const waiting = call(() => WgpuCanvasContext.present(this.#contextId));
    this.#state.current = null;
    showWaiting(this.#contextId, waiting);
  }

  get [Symbol.toStringTag](): string {
    return "GPUCanvasContext";
  }
}

/**
 * `RNWebGPU.MakeWebGPUCanvasContext(contextId, width, height)`: a new
 * context object for canvas `contextId`, its canvas `width` × `height`
 * (the view's layout size in points, as the reference's).
 */
export function makeCanvasContext(contextId: number, width: number, height: number): GPUCanvasContext {
  const canvas = new CanvasElement(INTERNAL, contextId, width, height);
  return new GPUCanvasContext(INTERNAL, canvas, contextId);
}
