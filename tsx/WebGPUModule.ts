// react-native-webgpu's `WebGPUModule` (src/NativeWebGPUModule.ts, its
// TurboModule `{ install: () => boolean }`), area: react-parity. The
// TurboModule puts WebGPU in the JS runtime; in NSH the package's import
// does that (index.tsx calls installWebGPU()), so install() installs the
// globals again, which is idempotent, and reports success.

import { installWebGPU } from "./install";

/** The reference's TurboModule spec. */
export interface Spec {
  /** Installs WebGPU in the runtime; `true` once it is there. */
  install: () => boolean;
}

/** The module that installs WebGPU into the runtime. */
export const WebGPUModule: Spec = {
  install(): boolean {
    installWebGPU();
    return true;
  },
};
