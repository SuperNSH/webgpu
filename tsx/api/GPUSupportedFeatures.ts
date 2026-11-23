// GPUSupportedFeatures and WGSLLanguageFeatures: read-only sets of names.

import { internal } from "../internal/object";

/** A read-only set of names, as the spec's setlike interfaces are. */
export class GPUSupportedFeatures implements ReadonlySet<string> {
  readonly #names: Set<string>;

  constructor(token: unknown, names: Iterable<string>) {
    internal(token);
    this.#names = new Set(names);
  }

  /** How many there are. */
  get size(): number {
    return this.#names.size;
  }

  /** Whether `name` is one. */
  has(name: string): boolean {
    return this.#names.has(name);
  }

  /** Calls `callback` with each. */
  forEach(callback: (value: string, key: string, set: ReadonlySet<string>) => void, thisArg?: unknown): void {
    for (const name of this.#names) callback.call(thisArg, name, name, this);
  }

  /** `[name, name]` for each. */
  entries(): SetIterator<[string, string]> {
    return this.#names.entries();
  }

  /** The names. */
  keys(): SetIterator<string> {
    return this.#names.keys();
  }

  /** The names. */
  values(): SetIterator<string> {
    return this.#names.values();
  }

  [Symbol.iterator](): SetIterator<string> {
    return this.#names.values();
  }

  get [Symbol.toStringTag](): string {
    return "GPUSupportedFeatures";
  }
}

/** `navigator.gpu.wgslLanguageFeatures`. */
export class WGSLLanguageFeatures extends GPUSupportedFeatures {
  override get [Symbol.toStringTag](): string {
    return "WGSLLanguageFeatures";
  }
}
