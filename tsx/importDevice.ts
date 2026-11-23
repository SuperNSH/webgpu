// importDevice / adoptTexture (react-native-webgpu's src/importDevice.ts),
// area: react-parity. They wrap foreign Dawn handles (a WGPUDevice* /
// WGPUTexture* from react-native-skia's Graphite). nsh-webgpu runs on wgpu,
// whose objects are not Dawn's and have no C ABI pointer to adopt: both
// throw ERR_WEBGPU_UNSUPPORTED (docs/parity/react-parity.md).

import { unsupportedError } from "./internal/native";

/** A GPUDevice wrapping a Dawn `WGPUDevice*` made elsewhere. Unsupported on wgpu. */
export const importDevice = (pointer: bigint): GPUDevice => {
  void pointer;
  throw unsupportedError(
    "importDevice is unavailable: it wraps a Dawn WGPUDevice*, and nsh-webgpu runs on wgpu, which cannot adopt a Dawn device",
  );
};

/** A GPUTexture taking ownership of a Dawn `WGPUTexture*` made elsewhere. Unsupported on wgpu. */
export const adoptTexture = (pointer: bigint): GPUTexture => {
  void pointer;
  throw unsupportedError(
    "adoptTexture is unavailable: it adopts a Dawn WGPUTexture*, and nsh-webgpu runs on wgpu, which cannot adopt a Dawn texture",
  );
};
