# Offscreen canvas

`GPUOffscreenCanvas` renders without a view: a texture of the preferred canvas format that you draw into and read back. It is the reference's pure-JS class, on nsh-webgpu's WebGPU, and it is what to use in tests and for rendering to an image.

## Basic usage

```ts
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
```

(`examples/headless/samples/offscreen.ts`, type-checked. `examples/headless/react-parity.app.tsx`, `offscreenTriangle`, runs the same steps with the Triangle pipeline, and `react-parity.test.tsx` checks the result: `bgra8unorm 50x50`, 10000 bytes, red in the middle, transparent in the corner.)

`getImageData()` resolves `{ data, width, height, format }`; `data` is a plain array of bytes in the texture format's order, so swap red and blue for a `bgra8unorm` canvas.

## Differences from the reference

- `getImageData()` works for any width. The reference copies rows of `width * 4` bytes, which WebGPU accepts only when that is a multiple of 256 (widths that are multiples of 64). Here the rows are copied padded to 256 bytes and the padding dropped. For widths the reference supports, the result is the same.
- Otherwise as the reference: `getContext("webgpu")` returns the one context and `null` for any other name; `configure` ignores `config.format` and uses `navigator.gpu.getPreferredCanvasFormat()`; `getConfiguration()` throws `Method not implemented.`; `present()` does nothing; `unconfigure()` destroys the texture. All tested in `react-parity.test.tsx`.

A `<Canvas>` whose native view has not attached (or in a headless test) also renders offscreen; that is a different path, in `GPUCanvasContext` ([Canvas](canvas.md#frame-presentation)).
