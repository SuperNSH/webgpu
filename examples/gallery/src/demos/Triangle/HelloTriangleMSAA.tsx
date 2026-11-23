// apps/example/src/Triangle/HelloTriangleMSAA.tsx: 4x MSAA resolved into
// the canvas, one frame. As in the reference, the drawing buffer keeps the
// canvas's default size (the view's size in points).

import { cleanup, View } from "nsh";
import { Canvas } from "nsh-webgpu";
import { contextAfterLayout, useCanvas, useProbe } from "../../harness/useWebGPU";
import { redFragWGSL, triangleVertWGSL } from "./triangle";

export function HelloTriangleMSAA() {
  const probe = useProbe();
  let device: GPUDevice | undefined;
  cleanup(() => device?.destroy());
  const ref = useCanvas(async (canvasRef, isGone) => {
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) {
      throw new Error("No adapter");
    }
    device = probe.watch(await adapter.requestDevice());
    const gpuDevice = device;
    const presentationFormat = navigator.gpu.getPreferredCanvasFormat();

    const context = await contextAfterLayout(canvasRef, isGone);
    if (!context) {
      throw new Error("No context");
    }
    probe.context(context);
    context.configure({
      device,
      format: presentationFormat,
      alphaMode: "premultiplied",
    });
    const { canvas } = context;

    const sampleCount = 4;

    const pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: {
        module: device.createShaderModule({
          code: triangleVertWGSL,
        }),
      },
      fragment: {
        module: device.createShaderModule({
          code: redFragWGSL,
        }),
        targets: [
          {
            format: presentationFormat,
          },
        ],
      },
      primitive: {
        topology: "triangle-list",
      },
      multisample: {
        count: sampleCount,
      },
    });

    const texture = device.createTexture({
      size: [canvas.width, canvas.height],
      sampleCount,
      format: presentationFormat,
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    });
    const view = texture.createView();

    function frame() {
      const commandEncoder = gpuDevice.createCommandEncoder();

      const renderPassDescriptor: GPURenderPassDescriptor = {
        colorAttachments: [
          {
            view,
            resolveTarget: context!.getCurrentTexture().createView(),
            clearValue: [0, 0, 0, 1],
            loadOp: "clear",
            storeOp: "discard",
          },
        ],
      };

      const passEncoder = commandEncoder.beginRenderPass(renderPassDescriptor);
      passEncoder.setPipeline(pipeline);
      passEncoder.draw(3);
      passEncoder.end();

      gpuDevice.queue.submit([commandEncoder.finish()]);
      probe.frame(gpuDevice, context!);
      context!.present();
    }

    frame();
  });

  return (
    <View style={{ flex: 1 }}>
      <Canvas ref={ref} style={{ flex: 1 }} />
    </View>
  );
}
