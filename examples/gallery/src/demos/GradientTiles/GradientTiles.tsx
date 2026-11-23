// apps/example/src/GradientTiles/GradientTiles.tsx without TypeGPU (an npm
// package NSH's bundler does not resolve): the state is two signals, the
// uniform that mirrors them (`useMirroredUniform`) is a buffer written in an
// effect, and the full-screen pipeline is TypeGPU's `common.fullScreenTriangle`
// with the fragment function written in WGSL.

import { effect, signal, Text, View } from "nsh";
import { Button } from "nsh/base";
import { Canvas } from "nsh-webgpu";
import { StyleSheet } from "../../components/StyleSheet";
import { useWebGPU } from "../../harness/useWebGPU";

const gradientWGSL = /* wgsl */ `
struct VertexOutput {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
}

// TypeGPU's common.fullScreenTriangle.
@vertex
fn vertexMain(@builtin(vertex_index) vertexIndex: u32) -> VertexOutput {
  let pos = array<vec2f, 3>(vec2f(-1, -1), vec2f(3, -1), vec2f(-1, 3));
  let uv = array<vec2f, 3>(vec2f(0, 1), vec2f(2, 1), vec2f(0, -1));
  return VertexOutput(vec4f(pos[vertexIndex], 0, 1), uv[vertexIndex]);
}

@group(0) @binding(0) var<uniform> span: vec2u;

@fragment
fn fragmentMain(@location(0) uv: vec2f) -> @location(0) vec4f {
  let red = floor(uv.x * f32(span.x)) / f32(span.x);
  let green = floor(uv.y * f32(span.y)) / f32(span.y);
  return vec4f(red, green, 0.5, 1.0);
}
`;

export function GradientTiles() {
  const spanX = signal(4);
  const spanY = signal(4);

  const ref = useWebGPU(({ context, device, presentationFormat }) => {
    // Mirroring the state on the GPU as a uniform
    const span = device.createBuffer({ size: 8, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    effect(() => device.queue.writeBuffer(span, 0, new Uint32Array([spanX(), spanY()])));

    // Defining a full-screen shader
    const module = device.createShaderModule({ code: gradientWGSL });
    const pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: { module, entryPoint: "vertexMain" },
      fragment: { module, entryPoint: "fragmentMain", targets: [{ format: presentationFormat }] },
    });
    const bindGroup = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: span } }],
    });

    // Drawing to the canvas each frame
    return () => {
      const encoder = device.createCommandEncoder();
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          { view: context.getCurrentTexture().createView(), clearValue: [0, 0, 0, 0], loadOp: "clear", storeOp: "store" },
        ],
      });
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, bindGroup);
      pass.draw(3);
      pass.end();
      device.queue.submit([encoder.finish()]);
    };
  });

  const Step = (props: { title: string; testID: string; onPress: () => void }) => (
    <Button
      testID={props.testID}
      onPress={() => props.onPress()}
      class="rounded-(--radius) px-2 py-1 hover:bg-muted active:bg-muted focus:ring-2 focus:ring-ring"
    >
      <Text class="text-xl">{props.title}</Text>
    </Button>
  );

  return (
    <View style={style.container}>
      <Canvas ref={ref} style={style.webgpu} />
      <View style={style.controls}>
        <View style={style.buttonRow}>
          <Text style={style.spanText} class="text-foreground">
            span x: {spanX()}{" "}
          </Text>
          <Step testID="span-x-minus" title="➖" onPress={() => spanX.update((prev) => Math.max(1, prev - 1))} />
          <Step testID="span-x-plus" title="➕" onPress={() => spanX.update((prev) => Math.min(prev + 1, 10))} />
        </View>

        <View style={style.buttonRow}>
          <Text style={style.spanText} class="text-foreground">
            span y: {spanY()}{" "}
          </Text>
          <Step testID="span-y-minus" title="➖" onPress={() => spanY.update((prev) => Math.max(1, prev - 1))} />
          <Step testID="span-y-plus" title="➕" onPress={() => spanY.update((prev) => Math.min(prev + 1, 10))} />
        </View>
      </View>
    </View>
  );
}

const style = StyleSheet.create({
  container: {
    flex: 1,
  },
  webgpu: {
    aspectRatio: 1,
  },
  spanText: {
    fontSize: 20,
    fontWeight: "bold",
  },
  controls: {
    flex: 1,
    justifyContent: "center",
  },
  buttonRow: {
    flexDirection: "row",
    justifyContent: "center",
    alignItems: "center",
  },
});
