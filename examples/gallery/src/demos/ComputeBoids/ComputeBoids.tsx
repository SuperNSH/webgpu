// apps/example/src/ComputeBoids/ComputeBoids.tsx, rewritten without
// TypeGPU (an npm package NSH's bundler does not resolve): the same
// simulation, layouts, presets, palette and controls, with the functions
// TypeGPU compiles from TypeScript ("use gpu") written in WGSL by hand.
// TypeGPU's `dispatchThreads(boidCount)` is a guarded 64-wide dispatch, and
// `withColorAttachment({ view: ctx })` clears to transparent.

import { Text, View } from "nsh";
import { Button } from "nsh/base";
import { Canvas } from "nsh-webgpu";
import { StyleSheet } from "../../components/StyleSheet";
import { useWebGPU } from "../../harness/useWebGPU";

const colorPresets = {
  plumTree: [1.0, 2.0, 1.0],
  jeans: [2.0, 1.5, 1.0],
  greyscale: [0, 0, 0],
  hotcold: [0, 3.14, 3.14],
} as const;
type ColorPreset = keyof typeof colorPresets;

const presets = {
  default: {
    separationDistance: 0.05,
    separationStrength: 0.001,
    alignmentDistance: 0.3,
    alignmentStrength: 0.01,
    cohesionDistance: 0.3,
    cohesionStrength: 0.001,
  },
  mosquitos: {
    separationDistance: 0.02,
    separationStrength: 0.01,
    alignmentDistance: 0.0,
    alignmentStrength: 0.0,
    cohesionDistance: 0.177,
    cohesionStrength: 0.011,
  },
  blobs: {
    separationDistance: 0.033,
    separationStrength: 0.051,
    alignmentDistance: 0.047,
    alignmentStrength: 0.1,
    cohesionDistance: 0.3,
    cohesionStrength: 0.013,
  },
  particles: {
    separationDistance: 0.035,
    separationStrength: 1,
    alignmentDistance: 0.0,
    alignmentStrength: 0.0,
    cohesionDistance: 0.0,
    cohesionStrength: 0.0,
  },
} as const;
type PhysicsParams = (typeof presets)[keyof typeof presets];

const triangleSize = 0.03;

const boidCount = 1000;

const WORKGROUP_SIZE = 64;

const common = /* wgsl */ `
struct PhysicsParams {
  separationDistance: f32,
  separationStrength: f32,
  alignmentDistance: f32,
  alignmentStrength: f32,
  cohesionDistance: f32,
  cohesionStrength: f32,
}

struct BoidData {
  position: vec2f,
  velocity: vec2f,
}

const triangleSize = ${triangleSize};
`;

const simulateWGSL = /* wgsl */ `${common}
@group(0) @binding(0) var<uniform> params: PhysicsParams;
@group(0) @binding(1) var<storage, read> currentBoids: array<BoidData>;
@group(0) @binding(2) var<storage, read_write> nextBoids: array<BoidData>;

@compute @workgroup_size(${WORKGROUP_SIZE})
fn main(@builtin(global_invocation_id) id: vec3u) {
  let boidIdx = id.x;
  if (boidIdx >= ${boidCount}u) {
    return;
  }
  var instanceInfo = currentBoids[boidIdx];
  var separation = vec2f();
  var alignment = vec2f();
  var alignmentCount = 0u;
  var cohesion = vec2f();
  var cohesionCount = 0u;

  for (var i = 0u; i < arrayLength(&currentBoids); i++) {
    if (i == boidIdx) {
      continue;
    }
    let other = currentBoids[i];
    let dist = distance(instanceInfo.position, other.position);
    if (dist < params.separationDistance) {
      separation = separation + (instanceInfo.position - other.position);
    }
    if (dist < params.alignmentDistance) {
      alignment = alignment + other.velocity;
      alignmentCount++;
    }
    if (dist < params.cohesionDistance) {
      cohesion = cohesion + other.position;
      cohesionCount++;
    }
  }
  if (alignmentCount > 0u) {
    alignment = alignment / f32(alignmentCount);
  }
  if (cohesionCount > 0u) {
    cohesion = cohesion / f32(cohesionCount) - instanceInfo.position;
  }
  instanceInfo.velocity = instanceInfo.velocity +
    (separation * params.separationStrength +
      alignment * params.alignmentStrength +
      cohesion * params.cohesionStrength);
  instanceInfo.velocity = normalize(instanceInfo.velocity) *
    clamp(length(instanceInfo.velocity), 0.0, 0.01);

  if (instanceInfo.position[0] > 1.0 + triangleSize) {
    instanceInfo.position[0] = -1.0 - triangleSize;
  }
  if (instanceInfo.position[1] > 1.0 + triangleSize) {
    instanceInfo.position[1] = -1.0 - triangleSize;
  }
  if (instanceInfo.position[0] < -1.0 - triangleSize) {
    instanceInfo.position[0] = 1.0 + triangleSize;
  }
  if (instanceInfo.position[1] < -1.0 - triangleSize) {
    instanceInfo.position[1] = 1.0 + triangleSize;
  }
  instanceInfo.position = instanceInfo.position + instanceInfo.velocity;
  nextBoids[boidIdx] = instanceInfo;
}
`;

const renderWGSL = /* wgsl */ `${common}
@group(0) @binding(0) var<storage, read> boids: array<BoidData>;
@group(1) @binding(0) var<uniform> colorPalette: vec3f;

const triangleVertices = array<vec2f, 3>(
  vec2f(0, triangleSize),
  vec2f(-triangleSize / 2, -triangleSize / 2),
  vec2f(triangleSize / 2, -triangleSize / 2),
);

fn rotate(v: vec2f, angle: f32) -> vec2f {
  let pos = vec2f(
    v.x * cos(angle) - v.y * sin(angle),
    v.x * sin(angle) + v.y * cos(angle),
  );
  return pos;
}

fn getRotationFromVelocity(velocity: vec2f) -> f32 {
  return -atan2(velocity.x, velocity.y);
}

struct VertexOutput {
  @builtin(position) position: vec4f,
  @location(0) color: vec3f,
}

@vertex
fn vertexMain(@builtin(instance_index) boidIdx: u32, @builtin(vertex_index) vi: u32) -> VertexOutput {
  let boid = boids[boidIdx];
  let angle = getRotationFromVelocity(boid.velocity);
  let local = triangleVertices[vi];
  let rotated = rotate(local, angle);
  let pos = rotated + boid.position;
  let color = sin(colorPalette + angle) * 0.45 + 0.45;
  return VertexOutput(vec4f(pos, 0, 1), color);
}

@fragment
fn fragmentMain(@location(0) color: vec3f) -> @location(0) vec4f {
  return vec4f(color, 1);
}
`;

function randomizeBoids(device: GPUDevice, buffer: GPUBuffer) {
  const data = new Float32Array(boidCount * 4);
  for (let i = 0; i < boidCount; i++) {
    data.set(
      [Math.random() * 2 - 1, Math.random() * 2 - 1, Math.random() * 0.1 - 0.05, Math.random() * 0.1 - 0.05],
      i * 4,
    );
  }
  device.queue.writeBuffer(buffer, 0, data);
}

const paramsData = (params: PhysicsParams) =>
  new Float32Array([
    params.separationDistance,
    params.separationStrength,
    params.alignmentDistance,
    params.alignmentStrength,
    params.cohesionDistance,
    params.cohesionStrength,
  ]);

export function ComputeBoids() {
  // What the controls act on, once the scene has made it.
  let actions:
    | {
        randomize: () => void;
        setParams: (params: PhysicsParams) => void;
        updateColorPreset: (preset: ColorPreset) => void;
      }
    | undefined;

  const ref = useWebGPU(({ context, device, presentationFormat }) => {
    const colorPalette = device.createBuffer({
      label: "colorPalette",
      size: 16,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(colorPalette, 0, new Float32Array(colorPresets.jeans));
    const physicsParams = device.createBuffer({
      label: "physicsParams",
      size: 24,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(physicsParams, 0, paramsData(presets.default));

    // Storing the state in two buffers, and swapping between which one is the input and which one is the output.
    // Also known as "double-buffering"
    const boidBuffer = (label: string) =>
      device.createBuffer({
        label,
        size: boidCount * 16,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      });
    const boidsA = boidBuffer("boidsA");
    const boidsB = boidBuffer("boidsB");
    randomizeBoids(device, boidsA);

    const computePipeline = device.createComputePipeline({
      layout: "auto",
      compute: { module: device.createShaderModule({ code: simulateWGSL }) },
    });
    const renderModule = device.createShaderModule({ code: renderWGSL });
    const renderPipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: { module: renderModule, entryPoint: "vertexMain" },
      fragment: { module: renderModule, entryPoint: "fragmentMain", targets: [{ format: presentationFormat }] },
      primitive: { topology: "triangle-list" },
    });

    const renderGroup = (boids: GPUBuffer) =>
      device.createBindGroup({
        layout: renderPipeline.getBindGroupLayout(0),
        entries: [{ binding: 0, resource: { buffer: boids } }],
      });
    const renderGroupA = renderGroup(boidsA);
    const renderGroupB = renderGroup(boidsB);
    const paletteGroup = device.createBindGroup({
      layout: renderPipeline.getBindGroupLayout(1),
      entries: [{ binding: 0, resource: { buffer: colorPalette } }],
    });
    const computeGroup = (current: GPUBuffer, next: GPUBuffer) =>
      device.createBindGroup({
        layout: computePipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: physicsParams } },
          { binding: 1, resource: { buffer: current } },
          { binding: 2, resource: { buffer: next } },
        ],
      });
    const computeGroupA = computeGroup(boidsA, boidsB);
    const computeGroupB = computeGroup(boidsB, boidsA);

    // Used to swap between A and B variants of resources
    let even = false;

    actions = {
      randomize: () => randomizeBoids(device, even ? boidsB : boidsA),
      setParams: (params) => device.queue.writeBuffer(physicsParams, 0, paramsData(params)),
      updateColorPreset: (preset) => device.queue.writeBuffer(colorPalette, 0, new Float32Array(colorPresets[preset])),
    };

    return () => {
      even = !even;
      const encoder = device.createCommandEncoder();
      const computePass = encoder.beginComputePass();
      computePass.setPipeline(computePipeline);
      computePass.setBindGroup(0, even ? computeGroupA : computeGroupB);
      computePass.dispatchWorkgroups(Math.ceil(boidCount / WORKGROUP_SIZE));
      computePass.end();
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: context.getCurrentTexture().createView(),
            clearValue: [0, 0, 0, 0],
            loadOp: "clear",
            storeOp: "store",
          },
        ],
      });
      pass.setPipeline(renderPipeline);
      pass.setBindGroup(0, even ? renderGroupB : renderGroupA);
      pass.setBindGroup(1, paletteGroup);
      pass.draw(3, boidCount);
      pass.end();
      device.queue.submit([encoder.finish()]);
    };
  });

  const ControlButton = (props: { title: string; testID: string; onPress: () => void }) => (
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
            randomize:{" "}
          </Text>
          <ControlButton testID="boids-randomize" title="🔀" onPress={() => actions?.randomize()} />
        </View>
        <View style={style.buttonRow}>
          <Text style={style.spanText} class="text-foreground">
            presets:{" "}
          </Text>
          <ControlButton testID="boids-default" title="🐦" onPress={() => actions?.setParams(presets.default)} />
          <ControlButton testID="boids-mosquitos" title="🦟" onPress={() => actions?.setParams(presets.mosquitos)} />
          <ControlButton testID="boids-blobs" title="💧" onPress={() => actions?.setParams(presets.blobs)} />
          <ControlButton testID="boids-particles" title="⚛️" onPress={() => actions?.setParams(presets.particles)} />
        </View>
        <View style={style.buttonRow}>
          <Text style={style.spanText} class="text-foreground">
            colors:{" "}
          </Text>
          <ControlButton testID="boids-plum" title="🟪🟩" onPress={() => actions?.updateColorPreset("plumTree")} />
          <ControlButton testID="boids-jeans" title="🟦🟫" onPress={() => actions?.updateColorPreset("jeans")} />
          <ControlButton testID="boids-grey" title="⬛️⬜️" onPress={() => actions?.updateColorPreset("greyscale")} />
          <ControlButton testID="boids-hotcold" title="🟥🟦" onPress={() => actions?.updateColorPreset("hotcold")} />
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
