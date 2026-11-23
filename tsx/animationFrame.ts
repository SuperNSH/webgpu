// requestAnimationFrame / cancelAnimationFrame (area: surfaces-apple),
// on NSH's frame clock: NSH's globals of those names.

/** Calls `callback` before the next frame with its time in milliseconds. */
export const requestAnimationFrame = (callback: (timestamp: number) => void): number =>
  globalThis.requestAnimationFrame(callback);

/** Cancels a requested frame. */
export const cancelAnimationFrame = (handle: number): void => {
  globalThis.cancelAnimationFrame(handle);
};
