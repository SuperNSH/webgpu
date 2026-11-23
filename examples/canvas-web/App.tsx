// The web canvas (tsx/surface/web.tsx) in a browser: a
// `<canvas>` inside an NSH view, drawn with the browser's own WebGPU, as
// react-native-webgpu's web entry does. The reference's Triangle, plus a
// clear color that cycles, through `getContext("webgpu")` and `present()`.
//
// TSX only (no Rust), so it runs on the stock web runtime: the wasm build
// of nsh-webgpu's crate is not involved (docs/parity/surfaces-other.md).

/// <reference path="../../tsx/types/webgpu.d.ts" />
/// <reference path="../../tsx/types/dom.d.ts" />

import { mount, signal, Text, View } from "nsh";
import {
  allocateContextIdWeb,
  browserGPU,
  getNativeSurfaceWeb,
  isWeb,
  makeWebGPUCanvasContextWeb,
  WebCanvasView,
} from "../../tsx/surface/web";

const SHADER = `
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  var pos = array<vec2f, 3>(vec2(0.0, 0.6), vec2(-0.6, -0.6), vec2(0.6, -0.6));
  return vec4f(pos[i], 0.0, 1.0);
}
@fragment
fn fs() -> @location(0) vec4f {
  return vec4f(1.0, 0.0, 0.0, 1.0);
}`;

async function start(contextId: number, status: (text: string) => void) {
  const gpu = browserGPU();
  if (!gpu) {
    status(isWeb() ? "this browser has no WebGPU" : "not in a browser");
    return;
  }
  const adapter = await gpu.requestAdapter();
  if (!adapter) {
    status("no adapter");
    return;
  }
  const device = await adapter.requestDevice();
  const surface = getNativeSurfaceWeb(contextId);
  const context = makeWebGPUCanvasContextWeb(contextId, surface.width, surface.height);
  if (!context) {
    status("no webgpu context on the canvas");
    return;
  }
  const format = gpu.getPreferredCanvasFormat();
  context.configure({ device, format, alphaMode: "premultiplied" });
  const module = device.createShaderModule({ code: SHADER });
  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: { module, entryPoint: "vs" },
    fragment: { module, entryPoint: "fs", targets: [{ format }] },
    primitive: { topology: "triangle-list" },
  });
  let frame = 0;
  const draw = () => {
    const t = (frame % 120) / 120;
    const texture = context.getCurrentTexture();
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: texture.createView(),
          clearValue: [0.1, 0.2 + 0.5 * t, 0.7 - 0.5 * t, 1],
          loadOp: "clear",
          storeOp: "store",
        },
      ],
    });
    pass.setPipeline(pipeline);
    pass.draw(3);
    pass.end();
    device.queue.submit([encoder.finish()]);
    context.present();
    status(`browser WebGPU: ${texture.width}x${texture.height} ${format}, frame ${frame}`);
    frame++;
  };
  setInterval(draw, 50);
}

function App() {
  const contextId = allocateContextIdWeb();
  const status = signal("waiting for the canvas");
  return (
    <View class="flex-1 gap-4 bg-background p-4">
      <Text class="text-foreground" testID="status">
        {status()}
      </Text>
      <View class="flex-1">
        <WebCanvasView
          contextId={contextId}
          onCanvas={() => {
            start(contextId, (text) => status.set(text)).catch((error: unknown) =>
              status.set(`failed: ${String(error)}`),
            );
          }}
        />
      </View>
    </View>
  );
}

mount(App);
