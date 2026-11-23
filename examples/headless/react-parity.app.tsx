// The app react-parity.test.tsx opens (area: react-parity): one screen per
// `host.params.scenario`, each using react-native-webgpu's React API as an
// app would, and showing what it got as text for the test to read.

import { ErrorBoundary, host, mount, signal, Text, View, type JSX } from "nsh";
import {
  Canvas,
  createTestVideoFrame,
  GPUDeviceProvider,
  GPUOffscreenCanvas,
  type NativeBuffer,
  NativeBufferView,
  useCanvasRef,
  useDevice,
  useMainDevice,
  useNativeBufferSize,
  useSurface,
} from "nsh-webgpu";
import { Button } from "nsh/base";
import { redFragWGSL, triangleVertWGSL } from "./triangle";

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

function DeviceScreen() {
  const state = useDevice(undefined, { label: "hooked" });
  const canvas = useCanvasRef();
  const hooked = useSurface();
  return (
    <View class="flex-1 gap-2 p-4">
      <Text>{state.device ? `device: ${state.device.label}` : "device: pending"}</Text>
      <Text>{state.adapter ? `adapter: ${state.adapter.info.isFallbackAdapter ? "fallback" : "hardware"}` : "adapter: pending"}</Text>
      <Text>{canvas.current ? `canvas ref: ${typeof canvas.current.getContextId()}` : "canvas ref: none"}</Text>
      <Text>{hooked.surface ? `surface: ${hooked.surface.width}x${hooked.surface.height}` : "surface: none"}</Text>
      <View style={{ width: 7, height: 5 }}>
        <Canvas ref={canvas} style={{ flex: 1 }} />
      </View>
      <View style={{ width: 7, height: 5 }}>
        <Canvas ref={hooked.ref} style={{ flex: 1 }} />
      </View>
    </View>
  );
}

function Consumer() {
  const { adapter, device } = useMainDevice();
  return (
    <Text>{`main device: ${device ? device.label : "null"}, adapter: ${adapter ? "yes" : "null"}`}</Text>
  );
}

function Outside() {
  useMainDevice();
  return <Text>outside: no error</Text>;
}

function ProviderScreen() {
  return (
    <View class="flex-1 gap-2 p-4">
      <GPUDeviceProvider deviceDescriptor={{ label: "provided" }}>
        <Consumer />
        <Consumer />
      </GPUDeviceProvider>
      <ErrorBoundary fallback={(error) => <Text>{`outside: ${message(error)}`}</Text>}>
        <Outside />
      </ErrorBoundary>
    </View>
  );
}

const SIZE = 50; // not a multiple of 64: rows are padded on readback

/** The reference's triangle into a GPUOffscreenCanvas, then getImageData(). */
async function offscreenTriangle(): Promise<string> {
  const adapter = (await navigator.gpu.requestAdapter())!;
  const device = await adapter.requestDevice();
  const canvas = new GPUOffscreenCanvas(SIZE, SIZE);
  const context = canvas.getContext("webgpu")!;
  const format = navigator.gpu.getPreferredCanvasFormat();
  context.configure({ device, format });
  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: { module: device.createShaderModule({ code: triangleVertWGSL }), entryPoint: "main" },
    fragment: { module: device.createShaderModule({ code: redFragWGSL }), entryPoint: "main", targets: [{ format }] },
    primitive: { topology: "triangle-list" },
  });
  const encoder = device.createCommandEncoder();
  const pass = encoder.beginRenderPass({
    colorAttachments: [
      { view: context.getCurrentTexture().createView(), clearValue: [0, 0, 0, 0], loadOp: "clear", storeOp: "store" },
    ],
  });
  pass.setPipeline(pipeline);
  pass.draw(3);
  pass.end();
  device.queue.submit([encoder.finish()]);
  const image = await canvas.getImageData();
  const at = (x: number, y: number) => image.data.slice((y * SIZE + x) * 4, (y * SIZE + x) * 4 + 4).join(",");
  // bgra8unorm: red is byte 2.
  const red = format === "bgra8unorm" ? 2 : 0;
  let reds = 0;
  for (let i = 0; i < SIZE * SIZE; i++) if (image.data[i * 4 + red] === 255) reds++;
  return `offscreen: ${image.format} ${image.width}x${image.height} bytes=${image.data.length} center=${at(25, 25)} corner=${at(0, 0)} red=${reds}`;
}

/** Draws each pixel of `frame` (a texture) at its own position: a full-canvas triangle loading it. */
const COPY_WGSL = `
@group(0) @binding(0) var frame: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  var pos = array<vec2f, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
  return vec4f(pos[i], 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
  return textureLoad(frame, vec2u(p.xy), 0);
}`;

/**
 * createTestVideoFrame's bgra8 bytes drawn into a GPUOffscreenCanvas, then
 * read back. The canvas's texture is RENDER_ATTACHMENT | COPY_SRC |
 * TEXTURE_BINDING whatever `configure()` asks (the reference's
 * Offscreen.ts), so, as on the reference, the frame reaches it by a render
 * pass, not by a copy (which would need COPY_DST): the frame goes into a
 * texture of its own, which a pass draws pixel for pixel.
 */
async function offscreenFrame(): Promise<string> {
  const adapter = (await navigator.gpu.requestAdapter())!;
  const device = await adapter.requestDevice();
  const canvas = new GPUOffscreenCanvas(SIZE, 3);
  const context = canvas.getContext("webgpu")!;
  const format = navigator.gpu.getPreferredCanvasFormat();
  context.configure({ device, format });
  // In memory in nsh-webgpu: its bytes are in plane(i).
  const frame = createTestVideoFrame(SIZE, 3) as NativeBuffer & { plane(index?: number): Uint8Array };
  const source = device.createTexture({
    size: [SIZE, 3],
    format: "bgra8unorm",
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
  });
  device.queue.writeTexture({ texture: source }, frame.plane(), { bytesPerRow: SIZE * 4 }, [SIZE, 3]);
  const module = device.createShaderModule({ code: COPY_WGSL });
  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: { module, entryPoint: "vs" },
    fragment: { module, entryPoint: "fs", targets: [{ format }] },
  });
  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [{ binding: 0, resource: source.createView() }],
  });
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
  const image = await canvas.getImageData();
  // The frame is BGRA; an rgba8unorm canvas (Android's preferred) holds R and B swapped.
  const expected = frame.plane();
  const at = (i: number) => (format === "rgba8unorm" && i % 4 !== 1 && i % 4 !== 3 ? expected[i ^ 2] : expected[i]);
  const same = image.data.every((byte, i) => byte === at(i));
  frame.release();
  return `frame: ${image.format} same=${same} last=${image.data.slice(-4).join(",")}`;
}

function OffscreenScreen() {
  const triangle = signal("offscreen: rendering");
  const frame = signal("frame: rendering");
  offscreenTriangle().then(triangle.set, (error: unknown) => triangle.set(`offscreen: failed: ${message(error)}`));
  offscreenFrame().then(frame.set, (error: unknown) => frame.set(`frame: failed: ${message(error)}`));
  return (
    <View class="flex-1 gap-2 p-4">
      <Text>{triangle()}</Text>
      <Text>{frame()}</Text>
    </View>
  );
}

function BufferSizeScreen() {
  const tracked = useNativeBufferSize();
  const report = (width: number, height: number) => () => tracked.onPixelSizeChange({ width, height });
  return (
    <View class="flex-1 gap-2 p-4">
      <Text>{tracked.size ? `size: ${tracked.size.width}x${tracked.size.height}` : "size: none"}</Text>
      <Button testID="report-100" onPress={report(100, 50)} class="p-2">
        <Text>report 100x50</Text>
      </Button>
      <Button testID="report-200" onPress={report(200, 80)} class="p-2">
        <Text>report 200x80</Text>
      </Button>
      <Button testID="report-300" onPress={report(300, 90)} class="p-2">
        <Text>report 300x90</Text>
      </Button>
      <ErrorBoundary fallback={(error) => <Text>{`view: ${(error as { code?: string }).code}`}</Text>}>
        <NativeBufferView resizeMode="contain" onPixelSizeChange={tracked.onPixelSizeChange} />
      </ErrorBoundary>
    </View>
  );
}

const SCREENS: Record<string, () => JSX.Element> = {
  device: DeviceScreen,
  provider: ProviderScreen,
  offscreen: OffscreenScreen,
  buffersize: BufferSizeScreen,
};

function App() {
  const Screen = SCREENS[String(host.params.scenario ?? "device")] ?? DeviceScreen;
  return (
    <View class="flex-1 bg-background">
      <Screen />
    </View>
  );
}

mount(App);
