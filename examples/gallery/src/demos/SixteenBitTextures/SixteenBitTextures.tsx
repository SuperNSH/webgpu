// apps/example/src/SixteenBitTextures/SixteenBitTextures.tsx: the same
// gradient on an 8-bit canvas and a high-bit-depth one. The effect starts
// once both canvases are laid out; the probe captures the deep canvas.

import { cleanup, pixelRatio, Text, View } from "nsh";
import type { CanvasRef, RNCanvasContext } from "nsh-webgpu";
import {
  Canvas,
  cancelAnimationFrame,
  getPreferredHighBitDepthCanvasFormat,
  requestAnimationFrame,
} from "nsh-webgpu";
import { StyleSheet } from "../../components/StyleSheet";
import type { Probe } from "../../harness/probe";
import { contextAfterLayout, useCanvas, useProbe } from "../../harness/useWebGPU";

const PixelRatio = { get: () => pixelRatio() };

// rgba16float on iOS (16-bit float to the glass via the extended sRGB layer
// colorspace), rgb10a2unorm on Android (SurfaceFlinger quantizes SDR float16
// layers, but 10-bit buffers pass through composition).
const deepFormat = getPreferredHighBitDepthCanvasFormat();
const deepLabel = deepFormat === "rgb10a2unorm" ? "10-bit" : "16-bit";

// A dark-blue vertical gradient (the classic "night sky" worst case for
// banding) that slowly drifts up and down. The endpoints are integer 8-bit
// values with the same delta (+8/255) on every channel, so all three
// channels quantize at the same height: each band edge is a simultaneous
// r+g+b step, the strongest possible contour. On an 8-bit surface ~8 fat
// bands crawl across the screen; on a 16-bit float surface the gradient
// stays smooth. Both canvases must show the same colors.
const gradientWGSL = /* wgsl */ `
struct Uniforms {
  time: f32,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexOutput {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) vertexIndex: u32) -> VertexOutput {
  const pos = array<vec2f, 3>(
    vec2f(-1.0, -1.0),
    vec2f(3.0, -1.0),
    vec2f(-1.0, 3.0)
  );
  var out: VertexOutput;
  out.position = vec4f(pos[vertexIndex], 0.0, 1.0);
  out.uv = pos[vertexIndex] * 0.5 + 0.5;
  return out;
}

const colorA = vec3f(51.0, 56.0, 77.0) / 255.0;
const colorB = vec3f(59.0, 64.0, 85.0) / 255.0;

@fragment
fn fs(in: VertexOutput) -> @location(0) vec4f {
  let phase = 0.15 * sin(uniforms.time * 0.5);
  let t = clamp(in.uv.y + phase, 0.0, 1.0);
  let color = mix(colorA, colorB, t);
  return vec4f(color, 1.0);
}
`;

const makeRenderer = (
  device: GPUDevice,
  context: RNCanvasContext,
  format: GPUTextureFormat,
  uniformBuffer: GPUBuffer,
  probe?: Probe,
) => {
  probe?.context(context);
  const canvas = context.canvas;
  canvas.width = canvas.clientWidth * PixelRatio.get();
  canvas.height = canvas.clientHeight * PixelRatio.get();

  context.configure({
    device,
    format,
    alphaMode: "opaque",
  });

  const module = device.createShaderModule({ code: gradientWGSL });
  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: {
      module,
      entryPoint: "vs",
    },
    fragment: {
      module,
      entryPoint: "fs",
      targets: [{ format }],
    },
    primitive: {
      topology: "triangle-list",
    },
  });
  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [{ binding: 0, resource: { buffer: uniformBuffer } }],
  });

  return () => {
    const commandEncoder = device.createCommandEncoder();
    const passEncoder = commandEncoder.beginRenderPass({
      colorAttachments: [
        {
          view: context.getCurrentTexture().createView(),
          clearValue: [0, 0, 0, 1],
          loadOp: "clear",
          storeOp: "store",
        },
      ],
    });
    passEncoder.setPipeline(pipeline);
    passEncoder.setBindGroup(0, bindGroup);
    passEncoder.draw(3);
    passEncoder.end();

    device.queue.submit([commandEncoder.finish()]);
    probe?.frame(device, context);
    context.present();
  };
};

export function SixteenBitTextures() {
  const probe = useProbe();
  const ref8bit: { current: CanvasRef | null } = { current: null };
  const frameId: { current: number | null } = { current: null };
  let device: GPUDevice | undefined;
  cleanup(() => {
    if (frameId.current !== null) {
      cancelAnimationFrame(frameId.current);
    }
    device?.destroy();
  });
  // The reference's effect, run once the deep canvas (the second) exists.
  const ref16bit = useCanvas(async (canvas16, isGone) => {
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) {
      throw new Error("No adapter");
    }
    device = probe.watch(await adapter.requestDevice());
    const gpuDevice = device;
    const context8 = await contextAfterLayout(ref8bit.current!, isGone);
    const context16 = await contextAfterLayout(canvas16, isGone);
    if (!context8 || !context16) return;

    const uniformBuffer = device.createBuffer({
      size: 16,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const draw8bit = makeRenderer(device, context8, navigator.gpu.getPreferredCanvasFormat(), uniformBuffer);
    const draw16bit = makeRenderer(device, context16, deepFormat, uniformBuffer, probe);

    const render = () => {
      if (isGone()) return;
      const time = performance.now() / 1000;
      gpuDevice.queue.writeBuffer(uniformBuffer, 0, new Float32Array([time]));
      draw8bit();
      draw16bit();
      frameId.current = requestAnimationFrame(render);
    };
    frameId.current = requestAnimationFrame(render);
  });

  return (
    <View style={style.container}>
      <View style={style.column}>
        <Canvas ref={(canvas) => (ref8bit.current = canvas)} style={style.webgpu} />
        <Text style={style.label} class="text-foreground">8-bit</Text>
      </View>
      <View style={style.column}>
        <Canvas ref={ref16bit} style={style.webgpu} />
        <Text style={style.label} class="text-foreground">{deepLabel}</Text>
      </View>
    </View>
  );
}

const style = StyleSheet.create({
  container: {
    flex: 1,
    flexDirection: "row",
  },
  column: {
    flex: 1,
  },
  webgpu: {
    flex: 1,
  },
  label: {
    textAlign: "center",
    padding: 8,
    fontWeight: "bold",
  },
});
