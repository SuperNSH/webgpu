// The reference's Triangle (apps/example/src/Triangle/HelloTriangle.tsx) and
// Cube (apps/example/src/Cube/Cube.tsx), drawn into a <Canvas> context.
//
// The Cube is the reference's when the device has vertex buffers and bind
// groups (`setVertexBuffer`, `setBindGroup`); where they are not built yet
// (they throw ERR_WEBGPU_UNSUPPORTED), it draws the same cube, the same
// colors and the same rotation with its vertices and matrix in WGSL and the
// time passed as the first instance index.

import { cancelAnimationFrame, type RNCanvasContext, requestAnimationFrame } from "nsh-webgpu";

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

/** HelloTriangle: one frame, presented. */
export function drawTriangle(
  device: GPUDevice,
  context: RNCanvasContext,
  format: GPUTextureFormat,
  tap?: Tap,
): void {
  context.configure({ device, format, alphaMode: "premultiplied", usage: usage(tap) });
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
  const texture = context.getCurrentTexture();
  const textureView = texture.createView();
  const passEncoder = commandEncoder.beginRenderPass({
    colorAttachments: [{ view: textureView, clearValue: [0, 0, 0, 0], loadOp: "clear", storeOp: "store" }],
  });
  passEncoder.setPipeline(pipeline);
  passEncoder.draw(3);
  passEncoder.end();
  tap?.(commandEncoder, texture);
  device.queue.submit([commandEncoder.finish()]);
  context.present();
}

/**
 * Called with a frame's encoder and canvas texture before it is submitted
 * (the evidence capture, capture.ts); the canvas then has COPY_SRC.
 */
export type Tap = (encoder: GPUCommandEncoder, texture: GPUTexture) => void;

const usage = (tap: unknown) => GPUTextureUsage.RENDER_ATTACHMENT | (tap ? GPUTextureUsage.COPY_SRC : 0);

// The reference's cube (components/cube.ts): position, color, uv.
// prettier-ignore
export const cubeVertexArray = new Float32Array([
  1, -1, 1, 1,   1, 0, 1, 1,  0, 1,
  -1, -1, 1, 1,  0, 0, 1, 1,  1, 1,
  -1, -1, -1, 1, 0, 0, 0, 1,  1, 0,
  1, -1, -1, 1,  1, 0, 0, 1,  0, 0,
  1, -1, 1, 1,   1, 0, 1, 1,  0, 1,
  -1, -1, -1, 1, 0, 0, 0, 1,  1, 0,

  1, 1, 1, 1,    1, 1, 1, 1,  0, 1,
  1, -1, 1, 1,   1, 0, 1, 1,  1, 1,
  1, -1, -1, 1,  1, 0, 0, 1,  1, 0,
  1, 1, -1, 1,   1, 1, 0, 1,  0, 0,
  1, 1, 1, 1,    1, 1, 1, 1,  0, 1,
  1, -1, -1, 1,  1, 0, 0, 1,  1, 0,

  -1, 1, 1, 1,   0, 1, 1, 1,  0, 1,
  1, 1, 1, 1,    1, 1, 1, 1,  1, 1,
  1, 1, -1, 1,   1, 1, 0, 1,  1, 0,
  -1, 1, -1, 1,  0, 1, 0, 1,  0, 0,
  -1, 1, 1, 1,   0, 1, 1, 1,  0, 1,
  1, 1, -1, 1,   1, 1, 0, 1,  1, 0,

  -1, -1, 1, 1,  0, 0, 1, 1,  0, 1,
  -1, 1, 1, 1,   0, 1, 1, 1,  1, 1,
  -1, 1, -1, 1,  0, 1, 0, 1,  1, 0,
  -1, -1, -1, 1, 0, 0, 0, 1,  0, 0,
  -1, -1, 1, 1,  0, 0, 1, 1,  0, 1,
  -1, 1, -1, 1,  0, 1, 0, 1,  1, 0,

  1, 1, 1, 1,    1, 1, 1, 1,  0, 1,
  -1, 1, 1, 1,   0, 1, 1, 1,  1, 1,
  -1, -1, 1, 1,  0, 0, 1, 1,  1, 0,
  -1, -1, 1, 1,  0, 0, 1, 1,  1, 0,
  1, -1, 1, 1,   1, 0, 1, 1,  0, 0,
  1, 1, 1, 1,    1, 1, 1, 1,  0, 1,

  1, -1, -1, 1,  1, 0, 0, 1,  0, 1,
  -1, -1, -1, 1, 0, 0, 0, 1,  1, 1,
  -1, 1, -1, 1,  0, 1, 0, 1,  1, 0,
  1, 1, -1, 1,   1, 1, 0, 1,  0, 0,
  1, -1, -1, 1,  1, 0, 0, 1,  0, 1,
  -1, 1, -1, 1,  0, 1, 0, 1,  1, 0,
]);
const cubeVertexSize = 4 * 10;
const cubeVertexCount = 36;

const basicVertWGSL = `struct Uniforms {
  modelViewProjectionMatrix : mat4x4f,
}
@binding(0) @group(0) var<uniform> uniforms : Uniforms;

struct VertexOutput {
  @builtin(position) Position : vec4f,
  @location(0) fragUV : vec2f,
  @location(1) fragPosition: vec4f,
}

@vertex
fn main(
  @location(0) position : vec4f,
  @location(1) uv : vec2f
) -> VertexOutput {
  var output : VertexOutput;
  output.Position = uniforms.modelViewProjectionMatrix * position;
  output.fragUV = uv;
  output.fragPosition = 0.5 * (position + vec4(1.0, 1.0, 1.0, 1.0));
  return output;
}`;

const vertexPositionColorWGSL = `@fragment
fn main(
  @location(0) fragUV: vec2f,
  @location(1) fragPosition: vec4f
) -> @location(0) vec4f {
  return fragPosition;
}`;

// Column-major 4x4 matrices, as wgpu-matrix's.
type Mat4 = Float32Array;

function perspective(fovy: number, aspect: number, near: number, far: number): Mat4 {
  const f = 1 / Math.tan(fovy / 2);
  const range = 1 / (near - far);
  // prettier-ignore
  return new Float32Array([
    f / aspect, 0, 0, 0,
    0, f, 0, 0,
    0, 0, far * range, -1,
    0, 0, near * far * range, 0,
  ]);
}

function multiply(a: Mat4, b: Mat4): Mat4 {
  const out = new Float32Array(16);
  for (let c = 0; c < 4; c++) {
    for (let r = 0; r < 4; r++) {
      let sum = 0;
      for (let k = 0; k < 4; k++) sum += a[k * 4 + r] * b[c * 4 + k];
      out[c * 4 + r] = sum;
    }
  }
  return out;
}

/** translate(0, 0, -4) · rotate(axis, 1 rad), the reference's view matrix. */
function viewMatrix(axis: [number, number, number]): Mat4 {
  const len = Math.hypot(...axis);
  const [x, y, z] = axis.map((v) => v / len);
  const c = Math.cos(1);
  const s = Math.sin(1);
  const t = 1 - c;
  // prettier-ignore
  const rotation = new Float32Array([
    t * x * x + c, t * x * y + s * z, t * x * z - s * y, 0,
    t * x * y - s * z, t * y * y + c, t * y * z + s * x, 0,
    t * x * z + s * y, t * y * z - s * x, t * z * z + c, 0,
    0, 0, 0, 1,
  ]);
  // prettier-ignore
  const translation = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, -4, 1]);
  return multiply(translation, rotation);
}

function transformation(projection: Mat4, now: number): Mat4 {
  return multiply(projection, viewMatrix([Math.sin(now), Math.cos(now), 0]));
}

/** The cube in WGSL alone: vertices from the array, the matrix from the time in `instance_index` (milliseconds). */
const selfContainedCubeWGSL = (aspect: number) => `
const VERTICES = array<f32, ${cubeVertexArray.length}>(${Array.from(cubeVertexArray, (v) => v.toFixed(1)).join(", ")});

fn rotation(axis: vec3f, angle: f32) -> mat4x4f {
  let a = normalize(axis);
  let c = cos(angle);
  let s = sin(angle);
  let t = 1.0 - c;
  return mat4x4f(
    vec4f(t * a.x * a.x + c, t * a.x * a.y + s * a.z, t * a.x * a.z - s * a.y, 0.0),
    vec4f(t * a.x * a.y - s * a.z, t * a.y * a.y + c, t * a.y * a.z + s * a.x, 0.0),
    vec4f(t * a.x * a.z + s * a.y, t * a.y * a.z - s * a.x, t * a.z * a.z + c, 0.0),
    vec4f(0.0, 0.0, 0.0, 1.0));
}

struct VertexOutput {
  @builtin(position) Position : vec4f,
  @location(0) fragUV : vec2f,
  @location(1) fragPosition: vec4f,
}

@vertex
fn main(@builtin(vertex_index) index : u32, @builtin(instance_index) ms : u32) -> VertexOutput {
  let i = index * 10u;
  let position = vec4f(VERTICES[i], VERTICES[i + 1u], VERTICES[i + 2u], VERTICES[i + 3u]);
  let uv = vec2f(VERTICES[i + 8u], VERTICES[i + 9u]);
  let now = f32(ms % 6283185u) / 1000.0;
  let f = 1.0 / tan((2.0 * 3.14159265) / 10.0);
  let far = 100.0;
  let near = 1.0;
  let range = 1.0 / (near - far);
  let projection = mat4x4f(
    vec4f(f / ${aspect.toFixed(6)}, 0.0, 0.0, 0.0),
    vec4f(0.0, f, 0.0, 0.0),
    vec4f(0.0, 0.0, far * range, -1.0),
    vec4f(0.0, 0.0, near * far * range, 0.0));
  let translation = mat4x4f(
    vec4f(1.0, 0.0, 0.0, 0.0), vec4f(0.0, 1.0, 0.0, 0.0),
    vec4f(0.0, 0.0, 1.0, 0.0), vec4f(0.0, 0.0, -4.0, 1.0));
  let view = translation * rotation(vec3f(sin(now), cos(now), 0.0), 1.0);
  var output : VertexOutput;
  output.Position = projection * view * position;
  output.fragUV = uv;
  output.fragPosition = 0.5 * (position + vec4(1.0, 1.0, 1.0, 1.0));
  return output;
}`;

/** What a running scene reports. */
export interface CubeStatus {
  path: "reference" | "self-contained";
  frames: number;
}

/**
 * The reference's Cube: a render loop on requestAnimationFrame, presenting
 * each frame. Returns a function that stops it.
 */
export function runCube(
  device: GPUDevice,
  context: RNCanvasContext,
  format: GPUTextureFormat,
  onFrame: (status: CubeStatus) => void,
  tap?: (frame: number) => Tap | undefined,
): () => void {
  const canvas = context.canvas;
  context.configure({ device, format, alphaMode: "premultiplied", usage: usage(tap) });
  const depthTexture = device.createTexture({
    size: [canvas.width, canvas.height],
    format: "depth24plus",
    usage: GPUTextureUsage.RENDER_ATTACHMENT,
  });
  const aspect = canvas.width / canvas.height;
  const depthStencil: GPUDepthStencilState = {
    depthWriteEnabled: true,
    depthCompare: "less",
    format: "depth24plus",
  };
  const fragment = (code: string): GPUFragmentState => ({
    module: device.createShaderModule({ code }),
    targets: [{ format }],
  });

  let draw: (pass: GPURenderPassEncoder, now: number) => void;
  let path: CubeStatus["path"] = "reference";
  try {
    const verticesBuffer = device.createBuffer({
      size: cubeVertexArray.byteLength,
      usage: GPUBufferUsage.VERTEX,
      mappedAtCreation: true,
    });
    new Float32Array(verticesBuffer.getMappedRange()).set(cubeVertexArray);
    verticesBuffer.unmap();
    const pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: {
        module: device.createShaderModule({ code: basicVertWGSL }),
        buffers: [
          {
            arrayStride: cubeVertexSize,
            attributes: [
              { shaderLocation: 0, offset: 0, format: "float32x4" },
              { shaderLocation: 1, offset: 4 * 8, format: "float32x2" },
            ],
          },
        ],
      },
      fragment: fragment(vertexPositionColorWGSL),
      primitive: { topology: "triangle-list", cullMode: "back" },
      depthStencil,
    });
    const uniformBuffer = device.createBuffer({ size: 64, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    const bindGroup = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: uniformBuffer } }],
    });
    const projection = perspective((2 * Math.PI) / 5, aspect, 1, 100);
    // Probe the pass methods once: they throw ERR_WEBGPU_UNSUPPORTED where
    // they are not built.
    const probe = device.createCommandEncoder();
    const pass = probe.beginRenderPass({
      colorAttachments: [{ view: depthlessView(device, format), loadOp: "clear", storeOp: "store" }],
    });
    pass.setBindGroup(0, bindGroup);
    pass.setVertexBuffer(0, verticesBuffer);
    pass.end();
    draw = (p, now) => {
      const m = transformation(projection, now);
      device.queue.writeBuffer(uniformBuffer, 0, m.buffer, m.byteOffset, m.byteLength);
      p.setPipeline(pipeline);
      p.setBindGroup(0, bindGroup);
      p.setVertexBuffer(0, verticesBuffer);
      p.draw(cubeVertexCount);
    };
  } catch (error) {
    if ((error as { code?: string }).code !== "ERR_WEBGPU_UNSUPPORTED") throw error;
    path = "self-contained";
    const code = selfContainedCubeWGSL(aspect);
    const pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: { module: device.createShaderModule({ code }), entryPoint: "main" },
      fragment: fragment(vertexPositionColorWGSL),
      primitive: { topology: "triangle-list", cullMode: "back" },
      depthStencil,
    });
    draw = (p, now) => {
      p.setPipeline(pipeline);
      p.draw(cubeVertexCount, 1, 0, Math.floor((now * 1000) % 6283185));
    };
  }

  let frames = 0;
  let handle = 0;
  const render = () => {
    const now = Date.now() / 1000;
    const encoder = device.createCommandEncoder();
    const texture = context.getCurrentTexture();
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: texture.createView(),
          clearValue: [0, 0, 0, 0],
          loadOp: "clear",
          storeOp: "store",
        },
      ],
      depthStencilAttachment: {
        view: depthTexture.createView(),
        depthClearValue: 1.0,
        depthLoadOp: "clear",
        depthStoreOp: "store",
      },
    });
    draw(pass, now);
    pass.end();
    tap?.(frames)?.(encoder, texture);
    device.queue.submit([encoder.finish()]);
    context.present();
    frames += 1;
    onFrame({ path, frames });
    handle = requestAnimationFrame(render);
  };
  handle = requestAnimationFrame(render);
  return () => cancelAnimationFrame(handle);
}

/** A 1×1 color target for the probe pass. */
function depthlessView(device: GPUDevice, format: GPUTextureFormat): GPUTextureView {
  return device
    .createTexture({ size: [1, 1], format, usage: GPUTextureUsage.RENDER_ATTACHMENT })
    .createView();
}
