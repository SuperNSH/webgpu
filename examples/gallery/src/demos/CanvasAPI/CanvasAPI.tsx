// apps/example/src/CanvasAPI/CanvasAPI.tsx: the canvas driven imperatively
// from a button (React Native's <Button title> is the gallery's ActionButton).

import { cleanup, View } from "nsh";
import type { CanvasRef } from "nsh-webgpu";
import { Canvas } from "nsh-webgpu";
import { useProbe } from "../../harness/useWebGPU";
import { ActionButton } from "../../ui";

import { redFragWGSL, triangleVertWGSL } from "../Triangle/triangle";

export const CanvasAPI = () => {
  const probe = useProbe();
  const ref: { current: CanvasRef | null } = { current: null };
  const devices: GPUDevice[] = [];
  cleanup(() => devices.forEach((device) => device.destroy()));
  return (
    <View style={{ flex: 1 }}>
      <ActionButton
        testID="check-surface"
        onPress={() =>
          (async () => {
            const context = ref.current?.getContext("webgpu");
            if (!context) {
              throw new Error("No context");
            }
            const adapter = await navigator.gpu.requestAdapter();
            if (!adapter) {
              throw new Error("No adapter");
            }
            const device = probe.watch(await adapter.requestDevice());
            devices.push(device);
            probe.context(context);
            const presentationFormat = navigator.gpu.getPreferredCanvasFormat();

            const { canvas } = context;
            if (!context) {
              throw new Error("No context");
            }
            context.configure({
              device,
              format: presentationFormat,
              alphaMode: "premultiplied",
            });

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

            const commandEncoder = device.createCommandEncoder();

            const renderPassDescriptor: GPURenderPassDescriptor = {
              colorAttachments: [
                {
                  view,
                  resolveTarget: context.getCurrentTexture().createView(),
                  clearValue: [0, 0, 0, 1],
                  loadOp: "clear",
                  storeOp: "discard",
                },
              ],
            };

            const passEncoder =
              commandEncoder.beginRenderPass(renderPassDescriptor);
            passEncoder.setPipeline(pipeline);
            passEncoder.draw(3);
            passEncoder.end();

            device.queue.submit([commandEncoder.finish()]);
            probe.frame(device, context);
            context.present();
          })().catch((error: unknown) => probe.fail(error))
        }
        label="check surface"
      />
      <Canvas style={{ flex: 1 }} ref={(canvas) => (ref.current = canvas)} />
    </View>
  );
};
