# requestAnimationFrame

`requestAnimationFrame` and `cancelAnimationFrame` from `nsh-webgpu` are NSH's globals of the same names, re-exported so code written for react-native-webgpu imports them from the same place.

## Usage

```ts
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
```

(`examples/headless/samples/raf.ts`, type-checked, and seen failing `nsh check` when broken. The same loop drives the Cube in `examples/canvas-apple/scenes.ts`, `runCube`, on screen in the iOS simulator; `examples/headless/surfaces-apple.test.tsx` checks that three frames arrive.)

Stop the loop with the function it returns, for example in an NSH `cleanup()`, as `examples/canvas-apple/App.tsx` does.

## Why the reference has its own, and this one does not

react-native-webgpu ships its own `requestAnimationFrame` because React Native's global one drops frames on high refresh rate displays and does not exist on worklet runtimes. NSH's global is driven by NSH's frame clock (a display link on iOS), and NSH has no worklet runtimes, so nsh-webgpu passes NSH's through. All callbacks of one frame get the same timestamp, in milliseconds.

## Not checked

The frame rate on a 120 Hz display, and whether frames stop while the app is in the background (the reference's do). The Cube logged 277 and then 643 frames in the iOS simulator over its run; no rate was measured.
