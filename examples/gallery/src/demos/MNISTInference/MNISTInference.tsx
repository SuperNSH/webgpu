// apps/example/src/MNISTInference/MNISTInference.tsx without
// react-native-skia, gesture-handler, reanimated and worklets: the drawing
// pad is a WebGPU canvas (the same picture: the rgb(239, 239, 248) fill,
// the rgb(209, 209, 209) 28x28 grid, the drawn pixels scaled up nearest),
// strokes come from touch events and are rasterized into the 28x28 alpha
// image the reference reads back from its Skia surface (a 1-pixel-wide
// anti-aliased stroke), and the guess shows as text under the pad. The
// network (Lib.ts) is the reference's, on raw WebGPU compute.

import { signal, Text, View, windowSize } from "nsh";
import { Canvas } from "nsh-webgpu";
import { StyleSheet } from "../../components/StyleSheet";
import { useProbe, useWebGPU } from "../../harness/useWebGPU";
import { ActionButton } from "../../ui";

import type { Network } from "./Lib";
import { centerData, createDemo, SIZE } from "./Lib";

const padWGSL = /* wgsl */ `
struct VertexOutput {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VertexOutput {
  let pos = array<vec2f, 3>(vec2f(-1, -1), vec2f(3, -1), vec2f(-1, 3));
  let p = pos[i];
  return VertexOutput(vec4f(p, 0, 1), vec2f(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5));
}

struct Pad {
  // Device pixels per cell.
  cell: f32,
}

@group(0) @binding(0) var<storage, read> pixels: array<f32>;
@group(0) @binding(1) var<uniform> pad: Pad;

@fragment
fn fs(in: VertexOutput) -> @location(0) vec4f {
  let size = ${SIZE}.0;
  let cell = min(vec2u(in.uv * size), vec2u(${SIZE - 1}u));
  let fill = vec3f(239.0, 239.0, 248.0) / 255.0;
  let line = vec3f(209.0, 209.0, 209.0) / 255.0;
  // The grid: a 1-point line on each cell edge.
  let local = fract(in.uv * size) * pad.cell;
  let onLine = min(local.x, local.y) < 1.0;
  var color = select(fill, line, onLine);
  // The drawn image over it, black with the stroke's coverage.
  let alpha = pixels[cell.y * ${SIZE}u + cell.x];
  color = mix(color, vec3f(0.0), alpha);
  return vec4f(color, 1.0);
}
`;

/** Coverage of a 1-cell-wide stroke from `a` to `b` (cell units), max-blended into `image`. */
function strokeSegment(image: Float32Array, a: [number, number], b: [number, number]) {
  const minX = Math.max(0, Math.floor(Math.min(a[0], b[0]) - 1));
  const maxX = Math.min(SIZE - 1, Math.ceil(Math.max(a[0], b[0]) + 1));
  const minY = Math.max(0, Math.floor(Math.min(a[1], b[1]) - 1));
  const maxY = Math.min(SIZE - 1, Math.ceil(Math.max(a[1], b[1]) + 1));
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len2 = dx * dx + dy * dy;
  for (let y = minY; y <= maxY; y++) {
    for (let x = minX; x <= maxX; x++) {
      const px = x + 0.5;
      const py = y + 0.5;
      const t = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((px - a[0]) * dx + (py - a[1]) * dy) / len2));
      const dist = Math.hypot(px - (a[0] + t * dx), py - (a[1] + t * dy));
      const coverage = Math.max(0, Math.min(1, 1 - dist));
      const i = y * SIZE + x;
      image[i] = Math.max(image[i], coverage);
    }
  }
}

export function MNISTInference() {
  const probe = useProbe();
  const text = signal("");
  const status = signal("Loading the network…");
  const image = new Float32Array(SIZE * SIZE);
  let network: Network | undefined;
  let upload: (() => void) | undefined;
  let last: [number, number] | null = null;
  let running = false;
  let pending = false;

  const width = () => windowSize().width;
  const cellSize = () => width() / SIZE;

  const runInference = async () => {
    if (network === undefined) {
      return;
    }
    if (running) {
      pending = true;
      return;
    }
    running = true;
    try {
      const data = centerData(Uint8Array.from(image, (v) => Math.round(v * 255))).map((x) => (x / 255) * 3.24 - 0.42);
      const certainties = await network.inference(data);
      const max = Math.max(...certainties);
      const index = certainties.indexOf(max);
      text.set(`${index}`);
    } catch (error) {
      probe.fail(error);
    } finally {
      running = false;
      if (pending) {
        pending = false;
        void runInference();
      }
    }
  };

  const ref = useWebGPU(async ({ context, device, presentationFormat, canvas }) => {
    const pixels = device.createBuffer({
      size: image.byteLength,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
    const padUniform = device.createBuffer({ size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    const module = device.createShaderModule({ code: padWGSL });
    const pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: { module, entryPoint: "vs" },
      fragment: { module, entryPoint: "fs", targets: [{ format: presentationFormat }] },
    });
    const bindGroup = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: pixels } },
        { binding: 1, resource: { buffer: padUniform } },
      ],
    });
    let dirty = true;
    upload = () => {
      dirty = true;
    };

    createDemo(device)
      .then((demo) => {
        network = demo.network;
        status.set("Draw a digit");
      })
      .catch((error: unknown) => status.set(`The network did not load: ${String(error)}`));

    return () => {
      if (dirty) {
        device.queue.writeBuffer(pixels, 0, image);
        device.queue.writeBuffer(padUniform, 0, new Float32Array([canvas.width / SIZE]));
        dirty = false;
      }
      const encoder = device.createCommandEncoder();
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          { view: context.getCurrentTexture().createView(), clearValue: [0, 0, 0, 1], loadOp: "clear", storeOp: "store" },
        ],
      });
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, bindGroup);
      pass.draw(3);
      pass.end();
      device.queue.submit([encoder.finish()]);
    };
  });

  const toCells = (x: number, y: number): [number, number] => [x / cellSize(), y / cellSize()];

  return (
    <View style={style.container}>
      <ActionButton
        testID="mnist-reset"
        label="Reset"
        onPress={() => {
          image.fill(0);
          upload?.();
          last = null;
          text.set("");
        }}
      />
      <View
        testID="mnist-pad"
        style={{ width: width(), height: width() }}
        onStartShouldSetResponder={() => true}
        onTouchStart={(e) => {
          last = toCells(e.locationX, e.locationY);
        }}
        onTouchMove={(e) => {
          const next = toCells(e.locationX, e.locationY);
          strokeSegment(image, last ?? next, next);
          last = next;
          upload?.();
          void runInference();
        }}
        onTouchEnd={() => {
          last = null;
        }}
      >
        <Canvas ref={ref} style={{ flex: 1 }} />
      </View>
      <Text testID="mnist-status" class="px-4 py-2 text-sm text-muted-foreground">
        {status()}
      </Text>
      <Text testID="mnist-guess" class="text-foreground" style={{ fontSize: 200, textAlign: "center" }}>
        {text()}
      </Text>
    </View>
  );
}

const style = StyleSheet.create({
  container: {
    flex: 1,
  },
});
