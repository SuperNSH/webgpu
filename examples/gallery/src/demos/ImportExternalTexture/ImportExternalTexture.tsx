// apps/example/src/ImportExternalTexture/ImportExternalTexture.tsx, partly:
// the same shader (`texture_external`, textureSampleBaseClampToEdge, 'cover'
// fit), pipeline and per-frame importExternalTexture, but the source is not
// a decoded video: createVideoPlayer throws ERR_WEBGPU_UNSUPPORTED in
// nsh-webgpu (no platform decoder, no IOSurface/AHardwareBuffer import in
// wgpu). The frame is createTestVideoFrame's BGRA test pattern (the
// reference's own test frame), written into a GPUTexture, which is the
// source nsh-webgpu's importExternalTexture takes (docs/parity/commands.md).

import { cleanup, pixelRatio, platform, signal, Text, View } from "nsh";
import { Canvas, createTestVideoFrame, type NativeBuffer } from "nsh-webgpu";
import { StyleSheet } from "../../components/StyleSheet";
import { useWebGPU } from "../../harness/useWebGPU";

const PixelRatio = { get: () => pixelRatio() };

// This is the SharedTextureMemory demo, rewritten to use
// GPUDevice.importExternalTexture instead of the manual
// importSharedTextureMemory + createTexture + beginAccess/endAccess dance.
//
// The visible result is identical (the same video frame stretched 'cover' over
// the canvas), but the sampling path differs: the source is bound as a
// `texture_external` and read with textureSampleBaseClampToEdge, and Dawn owns
// the shared-memory begin/end-access lifecycle internally. A GPUExternalTexture
// expires once the queue work that used it is submitted, so we re-import one
// every frame.
const SHADER = /* wgsl */ `
struct VsOut {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
};

struct Uniforms {
  // Per-axis scale applied to UVs *around the center* so that the canvas
  // samples a sub-rectangle of the texture matching the canvas aspect ratio.
  // 'cover' fit: one axis is 1.0, the other is canvasAR / textureAR (or its
  // reciprocal), whichever is < 1 — i.e. we crop on the longer axis.
  uvScale: vec2f,
};

@group(0) @binding(0) var srcTex: texture_external;
@group(0) @binding(1) var srcSampler: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
  // Full-screen triangle.
  var positions = array<vec2f, 3>(
    vec2f(-1.0, -3.0),
    vec2f(-1.0,  1.0),
    vec2f( 3.0,  1.0),
  );
  var uvs = array<vec2f, 3>(
    vec2f(0.0, 2.0),
    vec2f(0.0, 0.0),
    vec2f(2.0, 0.0),
  );
  var out: VsOut;
  out.position = vec4f(positions[vid], 0.0, 1.0);
  out.uv = uvs[vid];
  return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
  let uv = vec2f(0.5) + (in.uv - vec2f(0.5)) * u.uvScale;
  return textureSampleBaseClampToEdge(srcTex, srcSampler, uv);
}
`;


/** The test frame's size: a 16:9 "video". */
const FRAME_WIDTH = 640;
const FRAME_HEIGHT = 360;

export const ImportExternalTexture = () => {
  const error = signal<string | null>(null);
  let closeFrame = () => {};
  cleanup(() => closeFrame());

  const ref = useWebGPU(({ context, device, presentationFormat, canvas }) => {
    canvas.width = canvas.clientWidth * PixelRatio.get();
    canvas.height = canvas.clientHeight * PixelRatio.get();

    // In place of the video player: one test frame in a texture.
    // nsh-webgpu's frame lives in memory: its bytes are in plane(i).
    const frame = createTestVideoFrame(FRAME_WIDTH, FRAME_HEIGHT) as NativeBuffer & { plane(index?: number): Uint8Array };
    const frameTexture = device.createTexture({
      label: "test-video-frame",
      size: [frame.width, frame.height],
      format: "bgra8unorm",
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    device.queue.writeTexture(
      { texture: frameTexture },
      frame.plane(0),
      { bytesPerRow: frame.width * 4 },
      [frame.width, frame.height],
    );

    // The browser's importExternalTexture takes a VideoFrame or a <video>, no texture.
    let source: unknown = frameTexture;
    if (platform.os === "web") {
      const { VideoFrame } = globalThis as unknown as {
        VideoFrame: new (data: Uint8Array, init: object) => { close(): void };
      };
      const video = new VideoFrame(frame.plane(0), {
        format: "BGRX",
        codedWidth: frame.width,
        codedHeight: frame.height,
        timestamp: 0,
      });
      closeFrame = () => video.close();
      source = video;
    }

    const module = device.createShaderModule({ code: SHADER });
    const pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: { module, entryPoint: "vs_main" },
      fragment: {
        module,
        entryPoint: "fs_main",
        targets: [{ format: presentationFormat }],
      },
      primitive: { topology: "triangle-list" },
    });
    const sampler = device.createSampler({
      magFilter: "linear",
      minFilter: "linear",
    });
    // One persistent uniform buffer; rewritten whenever the frame dimensions
    // change.
    const uniformBuffer = device.createBuffer({
      size: 16, // vec2<f32> padded to 16-byte uniform alignment
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });

    // 'cover' fit: scale UVs around their center so the longer axis of the
    // texture is cropped to match the canvas aspect ratio.
    const computeUvScale = (texW: number, texH: number): [number, number] => {
      const canvasAR = canvas.width / canvas.height;
      const texAR = texW / texH;
      if (texAR > canvasAR) {
        // Texture is wider than the canvas: crop horizontally.
        return [canvasAR / texAR, 1];
      } else {
        // Texture is taller than (or equal to) the canvas: crop vertically.
        return [1, texAR / canvasAR];
      }
    };

    // The reference holds the latest decoded frame across rAF ticks; here
    // the frame never changes.
    const currentFrame = frame;
    let lastDims: [number, number] | null = null;

    return () => {

      // A GPUExternalTexture expires after each submit, so re-import one every
      // tick, even when sampling the same frame as last tick. Unlike on the
      // web, the shared-memory begin/end-access window is tied to this
      // wrapper's lifetime, so we destroy() it right after submit (below) to
      // release the surface promptly instead of waiting for GC.
      let externalTex: GPUExternalTexture | null = null;
      if (currentFrame) {
        try {
          externalTex = device.importExternalTexture({
            // nsh-webgpu's importExternalTexture source: a GPUTexture (a VideoFrame on the web).
            source: source as GPUExternalTextureDescriptor["source"],
            label: "video-frame",
          });
        } catch (e) {
          // Rare: a driver or an emulator that cannot import native surfaces.
          // The error names what is missing; show it and stop.
          error.set(e instanceof Error ? e.message : String(e));
          throw e;
        }
      }

      const encoder = device.createCommandEncoder();
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: context.getCurrentTexture().createView(),
            clearValue: { r: 0, g: 0, b: 0, a: 1 },
            loadOp: "clear",
            storeOp: "store",
          },
        ],
      });

      if (externalTex && currentFrame) {
        if (
          !lastDims ||
          lastDims[0] !== currentFrame.width ||
          lastDims[1] !== currentFrame.height
        ) {
          const [sx, sy] = computeUvScale(
            currentFrame.width,
            currentFrame.height,
          );
          device.queue.writeBuffer(
            uniformBuffer,
            0,
            new Float32Array([sx, sy]),
          );
          lastDims = [currentFrame.width, currentFrame.height];
        }
        const bindGroup = device.createBindGroup({
          layout: pipeline.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: externalTex },
            { binding: 1, resource: sampler },
            { binding: 2, resource: { buffer: uniformBuffer } },
          ],
        });
        pass.setPipeline(pipeline);
        pass.setBindGroup(0, bindGroup);
        pass.draw(3);
      }

      pass.end();
      device.queue.submit([encoder.finish()]);
      // (useWebGPU presents.) Now that the work sampling it has been
      // submitted, end the external texture's access window.
      // (react-native-webgpu's GPUExternalTexture has destroy(); the spec's type does not.)
      queueMicrotask(() => (externalTex as unknown as { destroy(): void } | null)?.destroy());
    };
  });

  return (
    <View style={{ flex: 1 }}>
      <Canvas ref={ref} style={{ flex: 1 }} />
      <If when={error()}>
        {(message) => (
          <View style={styles.errorContainer}>
            <Text style={styles.errorText}>{message()}</Text>
          </View>
        )}
      </If>
    </View>
  );
};

const styles = StyleSheet.create({
  errorContainer: { position: "absolute", left: 0, right: 0, top: 0, bottom: 0, padding: 16, justifyContent: "center" },
  errorText: { color: "red", fontSize: 14 },
});
