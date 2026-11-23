// apps/example/src/ComputeToys/ComputeToy.tsx on NSH: state as signals,
// the effects run once the canvas exists, and the pan gesture
// (react-native-gesture-handler) is the view's touch events. The page's
// probe watches the engine's device and captures a frame before present.

import { cleanup, platform, resource, View, windowSize } from "nsh";
import type { CanvasRef, RNCanvasContext } from "nsh-webgpu";
import { Canvas, cancelAnimationFrame, requestAnimationFrame } from "nsh-webgpu";
import { useCanvas, useProbe } from "../../harness/useWebGPU";

import { ComputeEngine } from "./engine";

const Platform = { OS: platform.os };

export interface ComputeToy {
  shader: string;
  uniforms: Record<string, number>;
}

// Use a CORS proxy for web to bypass CORS restrictions
const getCorsProxyUrl = (url: string) => {
  return Platform.OS === "web"
    ? `https://corsproxy.io/?${encodeURIComponent(url)}`
    : url;
};

export const useComputeToy = (toyId: number) =>
  resource(async (): Promise<ComputeToy> => {
    {
      const shaderURL = getCorsProxyUrl(
        `https://compute.toys/view/${toyId}/wgsl`,
      );
      const uniformsURL = getCorsProxyUrl(
        `https://compute.toys/view/${toyId}/json`,
      );

      // Execute both fetch requests in parallel
      const [shaderResponse, uniformsResponse] = await Promise.all([
        fetch(shaderURL),
        fetch(uniformsURL),
      ]);

      // Process the responses in parallel
      const [shader, uniformsJSON] = await Promise.all([
        shaderResponse.text(),
        uniformsResponse.json(),
      ]);
      const uniforms: Record<string, number> = {};
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      uniformsJSON.body.uniforms.forEach((uniform: any) => {
        uniforms[uniform.name] = uniform.value;
      });
      return { shader, uniforms };
    }
  });

export interface ComputeToyProps {
  toy: ComputeToy;
}

export const ComputeToy = (props: ComputeToyProps) => {
  const { shader, uniforms } = props.toy;
  const probe = useProbe();
  const mouse = { value: { pos: { x: 0, y: 0 }, click: false } };
  // The reference reads useWindowDimensions() once per render; here once.
  const { width, height } = windowSize();
  let engine: ComputeEngine | null = null;
  const animationRef: { current: number | null } = { current: null };
  const lastTimeRef = { current: 0 };
  let gone = false;

  cleanup(() => {
    gone = true;
    if (animationRef.current !== null) {
      cancelAnimationFrame(animationRef.current);
    }
  });

  // Animation/render loop
  const renderLoop = (timestamp: number) => {
    if (!engine || gone) {
      return;
    }

    // Calculate time delta
    const delta = lastTimeRef.current ? (timestamp - lastTimeRef.current) / 1000 : 0;
    lastTimeRef.current = timestamp;

    // Update time uniforms
    engine.setTimeElapsed(timestamp / 1000);
    engine.setTimeDelta(delta);
    if (mouse) {
      engine.setMousePos(mouse.value.pos.x, mouse.value.pos.y);
      engine.setMouseClick(mouse.value.click);
    }
    // Render frame

    engine.render();
    // Schedule next frame
    animationRef.current = requestAnimationFrame(renderLoop);
  };

  // Initialize WebGPU and the compute engine
  const ref = useCanvas(async (canvasRef) => {
    // Create the compute engine
    await ComputeEngine.create();
    const eng = ComputeEngine.getInstance();
    const device = (eng as unknown as { device: GPUDevice }).device;
    probe.watch(device);

    // The canvas as the engine sees it, with the probe in its context.
    const probed: CanvasRef = {
      getContextId: () => canvasRef.getContextId(),
      getNativeSurface: () => canvasRef.getNativeSurface(),
      getContext: (name) => {
        const context = canvasRef.getContext(name);
        if (!context) return null;
        probe.context(context);
        const present = context.present.bind(context);
        (context as RNCanvasContext).present = () => {
          probe.frame(device, context);
          present();
        };
        return context;
      },
    };

    // Set the canvas surface
    eng.setSurface(probed);

    // Set the canvas size based on your CANVAS constants
    // TODO: use PixelRatio.get()?
    eng.resize(width, height, 1);

    // Initialize render state
    eng.reset();

    // Set callbacks for shader compilation
    eng.onSuccess((entryPoints) => {
      console.log("Shader compiled successfully with entry points:", entryPoints);
    });

    eng.onError((message, row, col) => {
      console.error(`Shader error at ${row}:${col} - ${message}`);
    });
    eng.setCustomFloats(Object.keys(uniforms), new Float32Array(Object.values(uniforms)));
    // Process and compile the shader
    const preprocessed = await eng.preprocess(shader);
    if (preprocessed) {
      await eng.compile(preprocessed);
    }

    engine = eng;
    renderLoop(new Date().getTime());
  });

  const pointer = (e: { pageX: number; pageY: number }, click: boolean) => {
    mouse.value = {
      pos: {
        x: e.pageX / width,
        y: e.pageY / height,
      },
      click,
    };
  };

  return (
    <View
      style={{ width, height }}
      onStartShouldSetResponder={() => true}
      onTouchMove={(e) => pointer(e, true)}
      onTouchEnd={(e) => pointer(e, false)}
    >
      <Canvas ref={ref} style={{ width, height }} />
    </View>
  );
};
