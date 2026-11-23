// apps/example/src/Triangle/HelloTriangle.tsx: one frame, presented.

import { cleanup, pixelRatio, View } from "nsh";
import { Canvas } from "nsh-webgpu";
import { contextAfterLayout, useCanvas, useProbe } from "../../harness/useWebGPU";
import { redFragWGSL, triangleVertWGSL } from "./triangle";

export function HelloTriangle() {
  const probe = useProbe();
  let device: GPUDevice | undefined;
  cleanup(() => device?.destroy());
  const ref = useCanvas(async (canvasRef, isGone) => {
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) {
      throw new Error("No adapter");
    }

    device = probe.watch(await adapter.requestDevice());
    const presentationFormat = navigator.gpu.getPreferredCanvasFormat();

    const context = await contextAfterLayout(canvasRef, isGone);
    if (!context) {
      throw new Error("No context");
    }
    probe.context(context);
    const canvas = context.canvas;
    canvas.width = canvas.clientWidth * pixelRatio();
    canvas.height = canvas.clientHeight * pixelRatio();

    context.configure({
      device,
      format: presentationFormat,
      alphaMode: "premultiplied",
    });

    const pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: {
        module: device.createShaderModule({
          code: triangleVertWGSL,
        }),
        entryPoint: "main",
      },
      fragment: {
        module: device.createShaderModule({
          code: redFragWGSL,
        }),
        entryPoint: "main",
        targets: [
          {
            format: presentationFormat,
          },
        ],
      },
      primitive: {
        topology: "triangle-list",
      },
    });

    const commandEncoder = device.createCommandEncoder();

    const textureView = context.getCurrentTexture().createView();

    const renderPassDescriptor: GPURenderPassDescriptor = {
      colorAttachments: [
        {
          view: textureView,
          clearValue: [0, 0, 0, 0],
          loadOp: "clear",
          storeOp: "store",
        },
      ],
    };

    const passEncoder = commandEncoder.beginRenderPass(renderPassDescriptor);
    passEncoder.setPipeline(pipeline);
    passEncoder.draw(3);
    passEncoder.end();

    device.queue.submit([commandEncoder.finish()]);
    probe.frame(device, context);
    context.present();
  });

  return (
    <View style={{ flex: 1 }}>
      <View style={{ flex: 1, backgroundColor: "#3498db" }} />
      <Canvas ref={ref} style={{ position: "absolute", left: 0, top: 0, right: 0, bottom: 0 }} />
    </View>
  );
}
