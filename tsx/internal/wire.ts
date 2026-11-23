// A descriptor as it crosses to Rust: a plain object (src/convert.rs).
// GPU objects become their ids, iterables arrays, `undefined` members go.

import { HANDLE, isHandled } from "./object";

/** `value` with every GPU object replaced by its id. */
export function wire(value: unknown): unknown {
  if (value === null || value === undefined || typeof value !== "object") return value;
  if (isHandled(value)) return value[HANDLE];
  if (ArrayBuffer.isView(value)) return Array.from(value as unknown as ArrayLike<number>);
  if (Array.isArray(value)) return value.map(wire);
  if (Symbol.iterator in value) return Array.from(value as Iterable<unknown>, wire);
  const out: Record<string, unknown> = {};
  for (const key in value) {
    const member = (value as Record<string, unknown>)[key];
    if (member !== undefined) out[key] = wire(member);
  }
  return out;
}
