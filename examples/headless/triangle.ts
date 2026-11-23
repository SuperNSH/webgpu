// The reference's Triangle (apps/example/src/Triangle/HelloTriangle.tsx),
// offscreen: the same calls through nsh-webgpu's classes, into a texture
// of the preferred canvas format instead of the canvas, then read back.

import "nsh-webgpu";

export const triangleVertWGSL = `@vertex
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

export const redFragWGSL = `@fragment
fn main() -> @location(0) vec4f {
  return vec4(1.0, 0.0, 0.0, 1.0);
}`;

export const SIZE = 64;

/** The rendered pixels as RGBA rows, and the format they were drawn in. */
export interface Rendered {
  format: GPUTextureFormat;
  rgba: Uint8Array;
}

/** Renders the triangle and reads it back. */
export async function renderTriangle(): Promise<Rendered> {
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) {
    throw new Error("No adapter");
  }
  const device = await adapter.requestDevice();
  const presentationFormat = navigator.gpu.getPreferredCanvasFormat();

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

  // The canvas's current texture, offscreen.
  const texture = device.createTexture({
    size: [SIZE, SIZE],
    format: presentationFormat,
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
  });
  const commandEncoder = device.createCommandEncoder();
  const textureView = texture.createView();

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

  // Instead of context.present(): copy to a buffer and map it.
  const bytesPerRow = SIZE * 4;
  const readback = device.createBuffer({
    size: bytesPerRow * SIZE,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });
  commandEncoder.copyTextureToBuffer({ texture }, { buffer: readback, bytesPerRow }, [SIZE, SIZE]);
  device.queue.submit([commandEncoder.finish()]);
  await readback.mapAsync(GPUMapMode.READ);
  const bytes = new Uint8Array(readback.getMappedRange().slice(0));
  readback.unmap();
  if (presentationFormat.startsWith("bgra")) {
    for (let i = 0; i < bytes.length; i += 4) {
      const b = bytes[i];
      bytes[i] = bytes[i + 2];
      bytes[i + 2] = b;
    }
  }
  device.destroy();
  return { format: presentationFormat, rgba: bytes };
}

/** The pixel at (x, y) as "r,g,b,a". */
export function pixel(rgba: Uint8Array, x: number, y: number): string {
  const i = (y * SIZE + x) * 4;
  return Array.from(rgba.subarray(i, i + 4)).join(",");
}

/** How many pixels are opaque red. */
export function redCount(rgba: Uint8Array): number {
  let n = 0;
  for (let i = 0; i < rgba.length; i += 4) {
    if (rgba[i] === 255 && rgba[i + 1] === 0 && rgba[i + 2] === 0 && rgba[i + 3] === 255) n++;
  }
  return n;
}
