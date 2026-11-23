// WebIDL integer conversions shared by every spec class.

/** The largest `[EnforceRange] unsigned long` (GPUSize32, GPUIndex32, flags, ...). */
export const U32_MAX = 0xffffffff;
/** The largest `[EnforceRange] unsigned long long` JS can hold exactly (GPUSize64). */
export const U64_MAX = Number.MAX_SAFE_INTEGER;

/**
 * WebIDL `[EnforceRange]` integer conversion, as the spec's IDL says for
 * nearly every integer it takes: NaN, Infinity or a value outside
 * `[0, max]` is a TypeError; a fraction is truncated toward zero.
 */
export function enforceRange(value: unknown, what: string, max: number = U64_MAX): number {
  const n = Number(value);
  if (!Number.isFinite(n)) throw new TypeError(`${what}: ${String(value)} is not a finite integer`);
  const t = Math.trunc(n);
  if (t < 0 || t > max) throw new TypeError(`${what}: ${String(value)} is outside [0, ${max}]`);
  return t + 0;
}

/** `enforceRange` for an optional argument: `undefined` stays `undefined`. */
export function optionalRange(value: unknown, what: string, max: number = U64_MAX): number | undefined {
  return value === undefined ? undefined : enforceRange(value, what, max);
}

/**
 * A copy of dictionary `dict` (a TypeError when it is not an object) with
 * the integer members `members` converted by `enforceRange` (each `[name,
 * max]`); a missing member stays missing, a required one is the caller's
 * to check.
 */
export function withIntegers<T extends object>(
  dict: T | undefined | null,
  what: string,
  members: readonly (readonly [keyof T & string, number])[],
): T {
  if (dict !== undefined && dict !== null && typeof dict !== "object") {
    throw new TypeError(`${what}: expected a dictionary, got ${String(dict)}`);
  }
  const out = { ...(dict ?? {}) } as Record<string, unknown>;
  for (const [name, max] of members) {
    if (out[name] !== undefined) out[name] = enforceRange(out[name], `${what}.${name}`, max);
  }
  return out as T;
}
