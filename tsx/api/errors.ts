// GPUError and its kinds, GPUPipelineError, GPUUncapturedErrorEvent,
// GPUDeviceLostInfo.

import { INTERNAL, internal } from "../internal/object";

/** A GPU error: what error scopes and `uncapturederror` report. */
export class GPUError {
  readonly #message: string;

  constructor(message: string) {
    if (new.target === GPUError) throw new TypeError("Illegal constructor");
    this.#message = String(message);
  }

  /** What went wrong. */
  get message(): string {
    return this.#message;
  }
}

/** A validation error: invalid use of the API. */
export class GPUValidationError extends GPUError {
  declare readonly __brand: "GPUValidationError";
}

/** An allocation the GPU could not make. */
export class GPUOutOfMemoryError extends GPUError {
  declare readonly __brand: "GPUOutOfMemoryError";
}

/** A failure of the implementation, not of the app's use. */
export class GPUInternalError extends GPUError {
  declare readonly __brand: "GPUInternalError";
}

/** Why `createRenderPipelineAsync` / `createComputePipelineAsync` rejected. */
export class GPUPipelineError extends DOMException {
  declare readonly __brand: "GPUPipelineError";
  readonly #reason: GPUPipelineErrorReason;

  constructor(message: string | undefined, options: GPUPipelineErrorInit) {
    super(message ?? "", "GPUPipelineError");
    this.#reason = options.reason;
  }

  /** `"validation"` or `"internal"`. */
  get reason(): GPUPipelineErrorReason {
    return this.#reason;
  }
}

/** The event a device dispatches for an error no scope caught. */
export class GPUUncapturedErrorEvent extends Event {
  declare readonly __brand: "GPUUncapturedErrorEvent";
  readonly #error: GPUError;

  constructor(type: string, init: { error: GPUError } & EventInit) {
    super(type, init);
    this.#error = init.error;
  }

  /** The error. */
  get error(): GPUError {
    return this.#error;
  }
}

/** Why a device was lost (`device.lost`). */
export class GPUDeviceLostInfo {
  declare readonly __brand: "GPUDeviceLostInfo";
  readonly #reason: GPUDeviceLostReason;
  readonly #message: string;

  constructor(token: unknown, reason: GPUDeviceLostReason, message: string) {
    internal(token);
    this.#reason = reason;
    this.#message = message;
  }

  /** `"destroyed"` or `"unknown"`. */
  get reason(): GPUDeviceLostReason {
    return this.#reason;
  }

  /** What the implementation said. */
  get message(): string {
    return this.#message;
  }
}

/** The GPUError of native kind `kind` (`src/error.rs` GpuErrorKind). */
export function gpuError(kind: string, message: string): GPUError {
  switch (kind) {
    case "out-of-memory":
      return new GPUOutOfMemoryError(message);
    case "internal":
      return new GPUInternalError(message);
    default:
      return new GPUValidationError(message);
  }
}

/** A lost-info object (for GPUDevice). */
export function lostInfo(reason: string, message: string): GPUDeviceLostInfo {
  return new GPUDeviceLostInfo(INTERNAL, reason === "destroyed" ? "destroyed" : "unknown", message);
}
