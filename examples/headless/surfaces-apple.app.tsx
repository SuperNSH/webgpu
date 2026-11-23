// The <Canvas> and its GPUCanvasContext through the UI, headless (area:
// surfaces-apple): a 64×64 canvas whose native view no host registers here,
// so the context renders offscreen, as it does before a view attaches. Each
// check shows its result as a text `<name>: <value>` that
// surfaces-apple.test.tsx reads.

import { mount, signal, Text, View } from "nsh";
import { Canvas, type CanvasRef, requestAnimationFrame, type RNCanvasContext } from "nsh-webgpu";
import { redFragWGSL, triangleVertWGSL } from "./triangle";

const SIZE = 64;

function App() {
  const results = signal<string[]>([]);
  const report = (name: string, value: unknown) =>
    results.set([...results(), `${name}: ${typeof value === "string" ? value : JSON.stringify(value)}`]);
  let mounted: (canvas: CanvasRef) => void = () => {};
  // The ref arrives once the canvas is mounted and laid out.
  const canvasReady = new Promise<CanvasRef>((resolve) => {
    mounted = resolve;
  });

  const thrown = (f: () => unknown): string => {
    try {
      f();
      return "none";
    } catch (error) {
      return `${(error as Error).name}: ${(error as Error).message}`;
    }
  };

  async function run() {
    const ref = await canvasReady;
    const adapter = await navigator.gpu.requestAdapter();
    const device = await adapter!.requestDevice();
    const format = navigator.gpu.getPreferredCanvasFormat();
    report("contextId", typeof ref.getContextId());
    report("unsupported context", thrown(() => ref.getContext("2d" as "webgpu")));
    report("native surface", String(ref.getNativeSurface().surface));
    const context = ref.getContext("webgpu") as RNCanvasContext;
    report("instance", context instanceof GPUCanvasContext);
    report("canvas size", `${context.canvas.width}x${context.canvas.height}`);
    report("configuration before", context.getConfiguration());
    report("texture before configure", thrown(() => context.getCurrentTexture()));
    report(
      "bad colorSpace",
      thrown(() => context.configure({ device, format, colorSpace: "rec2020" as PredefinedColorSpace })),
    );
    report(
      "bad toneMapping",
      thrown(() =>
        context.configure({ device, format, toneMapping: { mode: "loud" as GPUCanvasToneMappingMode } }),
      ),
    );
    report("bad format", thrown(() => context.configure({ device, format: "r8unorm" })));

    context.configure({ device, format, alphaMode: "premultiplied", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC });
    const configuration = context.getConfiguration()!;
    report("configuration", {
      device: configuration.device === device,
      format: configuration.format,
      usage: configuration.usage,
      viewFormats: configuration.viewFormats,
      colorSpace: configuration.colorSpace,
      toneMapping: configuration.toneMapping,
      alphaMode: configuration.alphaMode,
    });
    // Another context object of the same canvas shares the swap chain.
    const other = ref.getContext("webgpu")!;
    report("shared configuration", other.getConfiguration()?.format === format);

    const first = context.getCurrentTexture();
    report("same texture in a frame", first === context.getCurrentTexture() && first === other.getCurrentTexture());
    report("texture", `${first.width}x${first.height} ${first.format} ${first.usage} ${first instanceof GPUTexture}`);

    // The reference's Triangle, into the canvas, read back.
    const pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: { module: device.createShaderModule({ code: triangleVertWGSL }), entryPoint: "main" },
      fragment: { module: device.createShaderModule({ code: redFragWGSL }), entryPoint: "main", targets: [{ format }] },
      primitive: { topology: "triangle-list" },
    });
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [{ view: first.createView(), clearValue: [0, 0, 0, 0], loadOp: "clear", storeOp: "store" }],
    });
    pass.setPipeline(pipeline);
    pass.draw(3);
    pass.end();
    const bytesPerRow = SIZE * 4;
    const readback = device.createBuffer({ size: bytesPerRow * SIZE, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    encoder.copyTextureToBuffer({ texture: first }, { buffer: readback, bytesPerRow }, [SIZE, SIZE]);
    device.queue.submit([encoder.finish()]);
    context.present();
    await readback.mapAsync(GPUMapMode.READ);
    const bytes = new Uint8Array(readback.getMappedRange().slice(0));
    readback.unmap();
    const at = (x: number, y: number) => {
      const i = (y * SIZE + x) * 4;
      const [b, g, r, a] = format.startsWith("bgra") ? bytes.subarray(i, i + 4) : [bytes[i + 2], bytes[i + 1], bytes[i], bytes[i + 3]];
      return `${r},${g},${b},${a}`;
    };
    report("triangle", `center=${at(32, 32)} corner=${at(0, 0)}`);

    const afterPresent = context.getCurrentTexture();
    report("new texture after present", afterPresent !== first);
    context.configure({ device, format });
    const afterConfigure = context.getCurrentTexture();
    report("new texture after configure", afterConfigure !== afterPresent);
    context.canvas.width = 16;
    const afterResize = context.getCurrentTexture();
    report("after resize", `${afterResize !== afterConfigure} ${afterResize.width}x${afterResize.height}`);
    context.unconfigure();
    report("configuration after unconfigure", context.getConfiguration());
    report("texture after unconfigure", thrown(() => context.getCurrentTexture()).split(":")[0]);

    // requestAnimationFrame: NSH's frame clock drives a render loop.
    context.configure({ device, format });
    let frames = 0;
    const loop = (time: number) => {
      if (typeof time !== "number") return;
      context.getCurrentTexture();
      context.present();
      frames += 1;
      if (frames < 3) requestAnimationFrame(loop);
      else report("animation frames", frames);
    };
    requestAnimationFrame(loop);
  }

  run().catch((error: unknown) => report("failed", String(error)));

  return (
    <View class="flex-1 bg-background p-4">
      <View style={{ width: SIZE, height: SIZE }}>
        <Canvas
          ref={(r) => {
            if (r) mounted(r);
          }}
          style={{ flex: 1 }}
        />
      </View>
      {results().map((line) => (
        <Text class="text-foreground">{line}</Text>
      ))}
    </View>
  );
}

mount(App);
