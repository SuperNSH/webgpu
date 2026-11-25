// The web: WebGPU is the browser's own, as react-native-webgpu's
// `main/index.web.tsx` and `WebPolyfillGPUModule.ts` have it. A TSX app on
// the web runs on the browser's JavaScript engine, so `navigator.gpu`, the
// `GPU*` classes and `<canvas>.getContext("webgpu")` are the browser's; no
// wasm and no wgpu are involved (area: surfaces-other).
//
// NSH has no `<canvas>` element and shows no registered native component on
// the web (its box stays empty), so the canvas view here is a `<View>` whose
// DOM element (`data-testid="rnwgpu-canvas-<id>"`, the reference's
// `contextIdToId`) gets a `<canvas id="rnwgpu-canvas-<id>">` filling it.
// `<Canvas>` (Canvas.tsx, area surfaces-apple) uses these on the web:
// `WebCanvasView` in place of the `webgpu.canvas` native component, and
// `getNativeSurfaceWeb` / `makeWebGPUCanvasContextWeb` /
// `allocateContextIdWeb` in place of the native module, as the reference's
// polyfill stands in for `RNWebGPU`.

import { platform, View, type JSX } from "nsh";

/**
 * A web canvas as a `NativeCanvas` (types.ts), with the DOM canvas's own
 * event methods, as the reference's polyfill gives it. Kept apart from
 * types.ts so this file imports nothing native (it runs without the crate).
 */
export interface WebNativeCanvas {
  surface: bigint;
  width: number;
  height: number;
  clientWidth: number;
  clientHeight: number;
  addEventListener: (...args: unknown[]) => void;
  removeEventListener: (...args: unknown[]) => void;
  dispatchEvent: (...args: unknown[]) => boolean;
  setPointerCapture: (...args: unknown[]) => void;
  releasePointerCapture: (...args: unknown[]) => void;
}

/** The browser's `GPUCanvasContext` with react-native-webgpu's `present()` (an `RNCanvasContext`). */
export type WebCanvasContext = GPUCanvasContext & { present: () => void };

/** Whether the app runs in a browser. */
export function isWeb(): boolean {
  return platform.os === "web";
}

/** The DOM id of context `contextId`'s canvas (the reference's `contextIdToId`). */
export function contextIdToId(contextId: number): string {
  return "rnwgpu-canvas-" + contextId;
}

// The few DOM members used here (NSH's types declare no DOM).
interface DomStyle {
  [name: string]: string;
}
interface DomElement {
  readonly clientWidth: number;
  readonly clientHeight: number;
  readonly style: DomStyle;
  id: string;
  appendChild(child: DomElement): void;
  getAttribute(name: string): string | null;
  setAttribute(name: string, value: string): void;
  getContext(name: "webgpu"): object | null;
  addEventListener(...args: unknown[]): void;
  removeEventListener(...args: unknown[]): void;
  dispatchEvent(...args: unknown[]): boolean;
  setPointerCapture(...args: unknown[]): void;
  releasePointerCapture(...args: unknown[]): void;
}
interface DomDocument {
  getElementById(id: string): DomElement | null;
  querySelector(selector: string): DomElement | null;
  createElement(tag: "canvas"): DomElement;
}
interface DomGlobals {
  document?: DomDocument;
  devicePixelRatio?: number;
  navigator?: { gpu?: GPU };
  parent?: DomGlobals;
}

const globals = globalThis as unknown as DomGlobals;

/**
 * The window whose document shows the app. NSH's web player runs the app's
 * modules in a realm of their own (a same-origin iframe, `browser/realm.js`)
 * while the views are elements of the page around it, so that is the
 * page's window: the parent when it is reachable, else this one.
 */
function page(): DomGlobals {
  try {
    const parent = globals.parent;
    if (parent && parent !== globals && parent.document) return parent;
  } catch {
    // A cross-origin parent: the app is not in NSH's realm.
  }
  return globals;
}

// Read when this module is evaluated: before the package entry's
// `installWebGPU()` puts nsh-webgpu's own `navigator.gpu` in place (the
// entry's imports, Canvas.tsx and this file, run before its body).
// The page's, so the device and the page's `<canvas>` share a realm.
const BROWSER_GPU: GPU | null = isWeb() ? (page().navigator?.gpu ?? globals.navigator?.gpu ?? null) : null;

/** Whether there is a page to put a `<canvas>` in (headless runs have none). */
export function hasPage(): boolean {
  return page().document !== undefined;
}

/** The browser's `navigator.gpu`, or `null` (no browser, or no WebGPU in it). */
export function browserGPU(): GPU | null {
  return BROWSER_GPU;
}

/**
 * Context `contextId`'s `<canvas>`: made in its view's element the first
 * time it is asked for, once that element is in the page. `null` before.
 */
export function webCanvas(contextId: number): DomElement | null {
  const document = page().document;
  if (!document) return null;
  const id = contextIdToId(contextId);
  const existing = document.getElementById(id);
  if (existing) return existing;
  const host = document.querySelector(`[data-testid="${id}"]`);
  if (!host) return null;
  const canvas = document.createElement("canvas");
  canvas.id = id;
  // Fills the view NSH lays out; the drawing buffer's size is set by
  // makeWebGPUCanvasContextWeb, as the reference's polyfill does.
  canvas.style.display = "block";
  canvas.style.position = "absolute";
  canvas.style.left = "0";
  canvas.style.top = "0";
  canvas.style.width = "100%";
  canvas.style.height = "100%";
  host.appendChild(canvas);
  return canvas;
}

/** `WebCanvasView`'s props: the context id, as `webgpu.canvas` takes it. */
export interface WebCanvasViewProps {
  /** The canvas context's id (`allocateContextIdWeb`). */
  contextId: number;
  /** Called once the `<canvas>` is in the page (on the view's first layout). */
  onCanvas?: () => void;
}

/** The web's `webgpu.canvas`: a view holding a `<canvas>` that fills it. */
export function WebCanvasView(props: WebCanvasViewProps): JSX.Element {
  // The first layout comes from the engine before the page has the view's
  // element (the DOM runtime applies the batch after), so the `<canvas>`
  // goes in once the element is there: looked for again each 16 ms, for a
  // second at most (no DOM at all, as in headless runs: never).
  let state: "idle" | "looking" | "made" = "idle";
  const look = (tries: number) => {
    if (webCanvas(props.contextId)) {
      state = "made";
      props.onCanvas?.();
    } else if (tries > 0 && page().document) {
      setTimeout(() => look(tries - 1), 16);
    } else {
      state = "idle";
    }
  };
  return (
    <View
      testID={contextIdToId(props.contextId)}
      style={{ flex: 1, alignSelf: "stretch" }}
      onLayout={() => {
        if (state !== "idle") return;
        state = "looking";
        look(60);
      }}
    />
  );
}

let nextContextId = 1;

/** A new context id (`RNWebGPU.allocateContextId` of the reference's polyfill). */
export function allocateContextIdWeb(): number {
  return nextContextId++;
}

/**
 * Context `contextId`'s canvas as a `NativeCanvas` (layout size in CSS
 * pixels, like the native view's in dp; the DOM canvas's own event methods).
 * Zero sizes before its view is in the page.
 */
export function getNativeSurfaceWeb(contextId: number): WebNativeCanvas {
  const canvas = webCanvas(contextId);
  const width = canvas?.clientWidth ?? 0;
  const height = canvas?.clientHeight ?? 0;
  const noop = () => {};
  return {
    surface: BigInt(contextId),
    width,
    height,
    clientWidth: width,
    clientHeight: height,
    addEventListener: canvas ? canvas.addEventListener.bind(canvas) : noop,
    removeEventListener: canvas ? canvas.removeEventListener.bind(canvas) : noop,
    dispatchEvent: canvas ? canvas.dispatchEvent.bind(canvas) : () => false,
    setPointerCapture: canvas ? canvas.setPointerCapture.bind(canvas) : noop,
    releasePointerCapture: canvas ? canvas.releasePointerCapture.bind(canvas) : noop,
  };
}

/**
 * The browser's WebGPU context of context `contextId`'s canvas, its drawing
 * buffer `width` × `height` CSS pixels times the device pixel ratio, with
 * react-native-webgpu's `present()` as a no-op (the browser presents).
 * `null` when the browser has no WebGPU or the canvas is not in the page.
 */
export function makeWebGPUCanvasContextWeb(
  contextId: number,
  width: number,
  height: number,
): WebCanvasContext | null {
  const canvas = webCanvas(contextId);
  if (!canvas) return null;
  const dpr = page().devicePixelRatio || 1;
  const pixelWidth = String(Math.round(width * dpr));
  const pixelHeight = String(Math.round(height * dpr));
  if (canvas.getAttribute("width") !== pixelWidth || canvas.getAttribute("height") !== pixelHeight) {
    canvas.setAttribute("width", pixelWidth);
    canvas.setAttribute("height", pixelHeight);
  }
  const context = canvas.getContext("webgpu");
  if (!context) return null;
  return Object.assign(context, { present: () => {} }) as unknown as WebCanvasContext;
}
