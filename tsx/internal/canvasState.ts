// What <Canvas> shares with the hooks without a public name: a signal on
// each CanvasRef that changes when what getNativeSurface() reads may have
// changed (the view laid out again, its surface attached), which
// useSurface() follows.

/** The key of a CanvasRef's surface version accessor (non-enumerable). */
export const SURFACE_VERSION: unique symbol = Symbol("nsh-webgpu.surfaceVersion");

/** Reads `canvas`'s surface version (tracked), when it is a <Canvas>'s. */
export function surfaceVersion(canvas: object): number {
  const read = (canvas as { [SURFACE_VERSION]?: () => number })[SURFACE_VERSION];
  return typeof read === "function" ? read() : 0;
}
