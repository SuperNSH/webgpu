// apps/example/src/Resize/Resize.tsx: the canvas's width animates (there with
// react-native-reanimated's withRepeat(withTiming(1, 4000 ms, inOut(ease)),
// -1, true)); here a signal driven by requestAnimationFrame with the same
// curve and period. The scene reallocates its MSAA target when the size changes.

import { cleanup, pixelRatio, signal, View, windowSize } from "nsh";
import { Canvas, cancelAnimationFrame, requestAnimationFrame } from "nsh-webgpu";

import { redFragWGSL, triangleVertWGSL } from "../Triangle/triangle";
import { useWebGPU } from "../../harness/useWebGPU";

const PixelRatio = { get: () => pixelRatio() };

// React Native's Easing.ease, a cubic bezier (0.42, 0, 1, 1).
function ease(x: number): number {
  const bezier = (t: number, a: number, b: number) => 3 * (1 - t) * (1 - t) * t * a + 3 * (1 - t) * t * t * b + t * t * t;
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 30; i++) {
    const mid = (lo + hi) / 2;
    if (bezier(mid, 0.42, 1) < x) lo = mid;
    else hi = mid;
  }
  return bezier((lo + hi) / 2, 0, 1);
}

// React Native's Easing.inOut(easing).
const inOut = (easing: (t: number) => number) => (t: number) =>
  t < 0.5 ? easing(t * 2) / 2 : 1 - easing((1 - t) * 2) / 2;

/** A value going 0 → 1 → 0 over 2 × `duration`, forever (withRepeat(..., -1, true)). */
export const useLoop = ({ duration }: { duration: number }) => {
  const progress = signal(0);
  const start = Date.now();
  const curve = inOut(ease);
  let handle = 0;
  const tick = () => {
    const t = ((Date.now() - start) / duration) % 2;
    progress.set(curve(t <= 1 ? t : 2 - t));
    handle = requestAnimationFrame(tick);
  };
  handle = requestAnimationFrame(tick);
  cleanup(() => cancelAnimationFrame(handle));
  return progress;
};

export const Resize = () => {
  const progress = useLoop({ duration: 4000 });
  const width = () => 20 + progress() * (windowSize().width - 20);
  const ref = useWebGPU(({ context, device, presentationFormat, canvas }) => {
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
    let renderTarget: GPUTexture | undefined;
    let renderTargetView: GPUTextureView;
    return () => {
      const currentWidth = canvas.clientWidth * PixelRatio.get();
      const currentHeight = canvas.clientHeight * PixelRatio.get();

      // The canvas size is animating via CSS.
      // When the size changes, we need to reallocate the render target.
      // We also need to set the physical size of the canvas to match the computed CSS size.
      if (
        (currentWidth !== canvas.width ||
          currentHeight !== canvas.height ||
          !renderTargetView) &&
        currentWidth &&
        currentHeight
      ) {
        if (renderTarget !== undefined) {
          // Destroy the previous render target
          renderTarget.destroy();
        }

        // Setting the canvas width and height will automatically resize the textures returned
        // when calling getCurrentTexture() on the context.
        canvas.width = currentWidth;
        canvas.height = currentHeight;

        // Resize the multisampled render target to match the new canvas size.
        renderTarget = device.createTexture({
          size: [canvas.width, canvas.height],
          sampleCount,
          format: presentationFormat,
          usage: GPUTextureUsage.RENDER_ATTACHMENT,
        });

        renderTargetView = renderTarget.createView();
      }

      if (renderTargetView) {
        const commandEncoder = device.createCommandEncoder();

        const renderPassDescriptor: GPURenderPassDescriptor = {
          colorAttachments: [
            {
              view: renderTargetView,
              resolveTarget: context.getCurrentTexture().createView(),
              clearValue: [0.5, 0.5, 0.5, 0.5],
              loadOp: "clear",
              storeOp: "store",
            },
          ],
        };

        const passEncoder =
          commandEncoder.beginRenderPass(renderPassDescriptor);
        passEncoder.setPipeline(pipeline);
        passEncoder.draw(3);
        passEncoder.end();

        device.queue.submit([commandEncoder.finish()]);
      }
    };
  });
  return (
    <View style={{ flex: 1, alignItems: "center" }}>
      <View style={{ width: width(), flex: 1, backgroundColor: "cyan" }}>
        <Canvas ref={ref} style={{ flex: 1 }} />
      </View>
    </View>
  );
};
