import { importDevice, warnIfNotHardwareAccelerated } from "nsh-webgpu";

export async function adapterChecked(): Promise<GPUAdapter> {
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("No appropriate GPUAdapter found.");
  warnIfNotHardwareAccelerated(adapter);
  return adapter;
}

export function tryImport(pointer: bigint): GPUDevice | null {
  try {
    return importDevice(pointer);
  } catch (error) {
    if ((error as { code?: string }).code === "ERR_WEBGPU_UNSUPPORTED") return null;
    throw error;
  }
}
