import { GPUOffscreenCanvas } from "nsh-webgpu";

export async function offscreen(device: GPUDevice, pipeline: GPURenderPipeline) {
  const canvas = new GPUOffscreenCanvas(50, 50);
  const context = canvas.getContext("webgpu")!;
  const format = navigator.gpu.getPreferredCanvasFormat();
  context.configure({ device, format });

  const encoder = device.createCommandEncoder();
  const pass = encoder.beginRenderPass({
    colorAttachments: [
      { view: context.getCurrentTexture().createView(), clearValue: [0, 0, 0, 0], loadOp: "clear", storeOp: "store" },
    ],
  });
  pass.setPipeline(pipeline);
  pass.draw(3);
  pass.end();
  device.queue.submit([encoder.finish()]);

  const image = await canvas.getImageData();
  // image.data: width * height * 4 bytes in `image.format` order (bgra8unorm on Apple).
  return image;
}
