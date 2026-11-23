import { cancelAnimationFrame, requestAnimationFrame, type RNCanvasContext } from "nsh-webgpu";

export function loop(device: GPUDevice, context: RNCanvasContext, draw: (encoder: GPUCommandEncoder, view: GPUTextureView, time: number) => void) {
  let handle = 0;
  const frame = (time: number) => {
    const encoder = device.createCommandEncoder();
    draw(encoder, context.getCurrentTexture().createView(), time);
    device.queue.submit([encoder.finish()]);
    context.present();
    handle = requestAnimationFrame(frame);
  };
  handle = requestAnimationFrame(frame);
  return () => cancelAnimationFrame(handle);
}
