// The native module (src/api) and how its errors cross.
//
// A native function fails with an Error whose message is
// "<Name>: <message>" (src/error.rs). `call` and `callAsync` rethrow it as
// the class the spec throws: TypeError, RangeError, a DOMException
// (OperationError, AbortError, InvalidStateError), or an Error with
// `code` "ERR_WEBGPU_UNSUPPORTED" for what nsh-webgpu cannot do.

export {
  Wgpu,
  WgpuAdapter,
  WgpuBindGroup,
  WgpuBindGroupLayout,
  WgpuBuffer,
  WgpuCanvasContext,
  WgpuCommandEncoder,
  WgpuComputePassEncoder,
  WgpuComputePipeline,
  WgpuDevice,
  WgpuExternalTexture,
  WgpuImageBitmap,
  WgpuOffscreenCanvas,
  WgpuPipelineLayout,
  WgpuQuerySet,
  WgpuQueue,
  WgpuRenderBundle,
  WgpuRenderBundleEncoder,
  WgpuRenderPassEncoder,
  WgpuRenderPipeline,
  WgpuSampler,
  WgpuShaderModule,
  WgpuTexture,
  WgpuTextureView,
} from "nsh-webgpu/native";

const NATIVE_ERROR =
  /^(TypeError|RangeError|OperationError|AbortError|InvalidStateError|ERR_WEBGPU_UNSUPPORTED): ([\s\S]*)$/;

/** An Error with `code` "ERR_WEBGPU_UNSUPPORTED": what nsh-webgpu cannot do (docs/parity.md). */
export function unsupportedError(message: string): Error {
  const error = new Error(`ERR_WEBGPU_UNSUPPORTED: ${message}`) as Error & { code: string };
  error.code = "ERR_WEBGPU_UNSUPPORTED";
  return error;
}

/** Throws ERR_WEBGPU_UNSUPPORTED for `what`. */
export function unsupported(what: string): never {
  throw unsupportedError(`${what} is not supported by nsh-webgpu (see docs/parity.md)`);
}

/** The exception the spec throws for native error `error`. */
export function translate(error: unknown): unknown {
  const match = error instanceof Error ? NATIVE_ERROR.exec(error.message) : null;
  if (!match) return error;
  const [, name, message] = match;
  switch (name) {
    case "TypeError":
      return new TypeError(message);
    case "RangeError":
      return new RangeError(message);
    case "ERR_WEBGPU_UNSUPPORTED":
      return unsupportedError(message);
    default:
      return new DOMException(message, name);
  }
}

/** Runs native call `f`, rethrowing its error as the spec's. */
export function call<T>(f: () => T): T {
  try {
    return f();
  } catch (error) {
    throw translate(error);
  }
}

/** Awaits native call `f`, rejecting with the spec's error. */
export async function callAsync<T>(f: () => Promise<T>): Promise<T> {
  try {
    return await f();
  } catch (error) {
    throw translate(error);
  }
}
