/** The global installWebGPU() puts there (react-native-webgpu's overloads). */
const createImageBitmap = (globalThis as unknown as {
  createImageBitmap: (image: unknown, ...rest: unknown[]) => Promise<ImageBitmap>;
}).createImageBitmap;

export async function upload(device: GPUDevice, png: Uint8Array): Promise<GPUTexture> {
  const bitmap = await createImageBitmap(png, { premultiplyAlpha: "none" });
  const texture = device.createTexture({
    size: [bitmap.width, bitmap.height],
    format: "rgba8unorm",
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
  });
  device.queue.copyExternalImageToTexture({ source: bitmap }, { texture }, [bitmap.width, bitmap.height]);
  bitmap.close();
  return texture;
}
