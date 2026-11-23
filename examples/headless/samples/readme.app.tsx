import { mount, pixelRatio, View } from "nsh";
import { Canvas, type CanvasRef } from "nsh-webgpu";

const triangleVertWGSL = `@vertex
fn main(
  @builtin(vertex_index) VertexIndex : u32
) -> @builtin(position) vec4f {
  var pos = array<vec2f, 3>(
    vec2(0.0, 0.5),
    vec2(-0.5, -0.5),
    vec2(0.5, -0.5)
  );

  return vec4f(pos[VertexIndex], 0.0, 1.0);
}`;

const redFragWGSL = `@fragment
fn main() -> @location(0) vec4f {
  return vec4(1.0, 0.0, 0.0, 1.0);
}`;

async function draw(canvas: CanvasRef) {
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("No adapter");
  const device = await adapter.requestDevice();
  const format = navigator.gpu.getPreferredCanvasFormat();

  // The canvas is laid out once its ref arrives: the context takes its size.
  const context = canvas.getContext("webgpu")!;
  context.canvas.width = context.canvas.clientWidth * pixelRatio();
  context.canvas.height = context.canvas.clientHeight * pixelRatio();
  context.configure({ device, format, alphaMode: "premultiplied" });

  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: { module: device.createShaderModule({ code: triangleVertWGSL }), entryPoint: "main" },
    fragment: {
      module: device.createShaderModule({ code: redFragWGSL }),
      entryPoint: "main",
      targets: [{ format }],
    },
    primitive: { topology: "triangle-list" },
  });

  const commandEncoder = device.createCommandEncoder();
  const passEncoder = commandEncoder.beginRenderPass({
    colorAttachments: [
      {
        view: context.getCurrentTexture().createView(),
        clearValue: [0, 0, 0, 0],
        loadOp: "clear",
        storeOp: "store",
      },
    ],
  });
  passEncoder.setPipeline(pipeline);
  passEncoder.draw(3);
  passEncoder.end();
  device.queue.submit([commandEncoder.finish()]);
  context.present();
}

function HelloTriangle() {
  return (
    <View style={{ flex: 1, backgroundColor: "#3498db" }}>
      {/* The ref gets the canvas once it is mounted, and null when it unmounts. */}
      <Canvas ref={(canvas) => canvas && void draw(canvas)} style={{ flex: 1 }} />
    </View>
  );
}

mount(HelloTriangle);
