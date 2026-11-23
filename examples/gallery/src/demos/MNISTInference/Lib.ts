// apps/example/src/MNISTInference/Lib.ts without TypeGPU (an npm package
// NSH's bundler does not resolve): the same network (dense layers with ReLU,
// weights and biases downloaded as .npy from TypeGPU's site at run time, as
// the reference does), with TypeGPU's guarded compute pipeline written in
// WGSL (64-wide workgroups, `i >= count` returns) and its buffers as plain
// GPUBuffers.

export const SIZE = 28;

/** Where the reference downloads the weights from. */
export const WEIGHTS_URL = "https://docs.swmansion.com/TypeGPU/assets/mnist-weights/";

// Definitions for the network

interface LayerData {
  shape: readonly [number] | readonly [number, number];
  buffer: GPUBuffer;
}

interface Layer {
  weights: GPUBuffer;
  biases: GPUBuffer;
  state: GPUBuffer;
}

export interface Network {
  layers: Layer[];
  input: GPUBuffer;
  output: GPUBuffer;

  inference(data: number[]): Promise<number[]>;
}

export const centerData = (data: Uint8Array) => {
  const mass = data.reduce((acc, value) => acc + value, 0);
  const x = data.reduce((acc, value, i) => acc + value * (i % SIZE), 0) / mass;
  const y =
    data.reduce((acc, value, i) => acc + value * Math.floor(i / SIZE), 0) /
    mass;

  const offsetX = Math.round(SIZE / 2 - x);
  const offsetY = Math.round(SIZE / 2 - y);

  const newData = Array.from({ length: SIZE * SIZE }, () => 0);
  for (let i = 0; i < SIZE; i++) {
    for (let j = 0; j < SIZE; j++) {
      const index = i * SIZE + j;
      const newIndex = (i + offsetY) * SIZE + j + offsetX;
      if (newIndex >= 0 && newIndex < SIZE * SIZE) {
        newData[newIndex] = data[index];
      }
    }
  }

  return newData;
};

const WORKGROUP_SIZE = 64;

const layerWGSL = /* wgsl */ `
@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(1) @binding(0) var<storage, read> weights: array<f32>;
@group(1) @binding(1) var<storage, read> biases: array<f32>;

fn relu(x: f32) -> f32 {
  return max(0, x);
}

@compute @workgroup_size(${WORKGROUP_SIZE})
fn main(@builtin(global_invocation_id) id: vec3u) {
  let i = id.x;
  if (i >= arrayLength(&output)) {
    return;
  }
  let inputSize = arrayLength(&input);
  let weightsOffset = i * inputSize;
  var sum = f32(0);

  for (var j = 0u; j < inputSize; j++) {
    sum += input[j] * weights[weightsOffset + j];
  }

  sum += biases[i];
  output[i] = relu(sum);
}
`;

export const createDemo = async (device: GPUDevice) => {
  const pipeline = device.createComputePipeline({
    layout: "auto",
    compute: { module: device.createShaderModule({ code: layerWGSL }) },
  });

  const storage = (size: number) =>
    device.createBuffer({
      size: size * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
    });

  /**
   * Creates a network from a list of pairs of weights and biases
   *
   * It automates the creation of state buffers that are used to store the intermediate results of the network
   * as well as the input layer buffer
   *
   * It provides an inference function that takes an array of input data and returns an array of output data
   */
  function createNetwork(layers: [LayerData, LayerData][]): Network {
    const buffers = layers.map(([weights, biases]) => {
      if (weights.shape[1] !== biases.shape[0]) {
        throw new Error(`Shape mismatch: ${weights.shape} and ${biases.shape}`);
      }

      return {
        weights: weights.buffer,
        biases: biases.buffer,
        state: storage(biases.shape[0]),
        count: biases.shape[0],
      };
    });

    const input = storage(layers[0][0].shape[0]);
    const output = buffers[buffers.length - 1].state;
    const outputCount = buffers[buffers.length - 1].count;
    const readback = device.createBuffer({
      size: outputCount * 4,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });

    const ioBindGroups = buffers.map((_, i) =>
      device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: i === 0 ? input : buffers[i - 1].state } },
          { binding: 1, resource: { buffer: buffers[i].state } },
        ],
      }),
    );

    const weightsBindGroups = buffers.map((layer) =>
      device.createBindGroup({
        layout: pipeline.getBindGroupLayout(1),
        entries: [
          { binding: 0, resource: { buffer: layer.weights } },
          { binding: 1, resource: { buffer: layer.biases } },
        ],
      }),
    );

    async function inference(data: number[]): Promise<number[]> {
      // verify the length of the data matches the input layer
      if (data.length !== layers[0][0].shape[0]) {
        throw new Error(`Data length ${data.length} does not match input shape ${layers[0][0].shape[0]}`);
      }

      device.queue.writeBuffer(input, 0, new Float32Array(data));

      // Run the network
      const encoder = device.createCommandEncoder();
      const pass = encoder.beginComputePass();
      pass.setPipeline(pipeline);
      for (let i = 0; i < buffers.length; i++) {
        pass.setBindGroup(0, ioBindGroups[i]);
        pass.setBindGroup(1, weightsBindGroups[i]);
        pass.dispatchWorkgroups(Math.ceil(buffers[i].count / WORKGROUP_SIZE));
      }
      pass.end();
      encoder.copyBufferToBuffer(output, 0, readback, 0, outputCount * 4);
      device.queue.submit([encoder.finish()]);

      // Read the output
      await readback.mapAsync(GPUMapMode.READ);
      const result = Array.from(new Float32Array(readback.getMappedRange().slice(0)));
      readback.unmap();
      return result;
    }

    return {
      layers: buffers,
      input,
      output,
      inference,
    };
  }

  const network = createNetwork(await downloadLayers());

  // #region Downloading weights & biases

  /**
   * Create a LayerData object from a layer ArrayBuffer
   *
   * The function extracts the header, shape and data from the layer
   * If there are any issues with the layer, an error is thrown
   *
   * Automatically creates appropriate buffer initialized with the data
   */
  function getLayerData(layer: ArrayBuffer): LayerData {
    const headerLen = new Uint16Array(layer.slice(8, 10));

    const header = new TextDecoder().decode(
      new Uint8Array(layer.slice(10, 10 + headerLen[0])),
    );

    // shape can be found in the header in the format: 'shape': (x, y) or 'shape': (x,) for bias
    const shapeMatch = header.match(/'shape': \((\d+), ?(\d+)?\)/);
    if (!shapeMatch) {
      throw new Error("Shape not found in header");
    }

    // To accommodate .npy weirdness - if we have a 2d shape we need to switch the order
    const X = Number.parseInt(shapeMatch[1], 10);
    const Y = Number.parseInt(shapeMatch[2], 10);
    const shape = Number.isNaN(Y) ? ([X] as const) : ([Y, X] as const);

    const data = new Float32Array(layer.slice(10 + headerLen[0]));

    // Verify the length of the data matches the shape
    if (data.length !== shape[0] * (shape[1] || 1)) {
      throw new Error(
        `Data length ${data.length} does not match shape ${shape}`,
      );
    }

    const buffer = device.createBuffer({
      size: data.byteLength,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      mappedAtCreation: true,
    });
    new Float32Array(buffer.getMappedRange()).set(data);
    buffer.unmap();

    return {
      shape,
      buffer,
    };
  }

  function downloadLayers(): Promise<[LayerData, LayerData][]> {
    const downloadLayer = async (fileName: string): Promise<LayerData> => {
      const buffer = await fetch(`${WEIGHTS_URL}${fileName}`).then((res) => res.arrayBuffer());

      return getLayerData(buffer);
    };

    return Promise.all(
      [0, 1, 2, 3, 4, 5, 6, 7].map((layer) =>
        Promise.all([downloadLayer(`layer${layer}.weight.npy`), downloadLayer(`layer${layer}.bias.npy`)]),
      ),
    );
  }

  // #endregion

  return { network };
};
