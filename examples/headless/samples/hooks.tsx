import { Text, View } from "nsh";
import { Canvas, GPUDeviceProvider, useCanvasRef, useDevice, useMainDevice, useSurface } from "nsh-webgpu";

export function DeviceScreen() {
  const state = useDevice();
  const ref = useCanvasRef();
  return (
    <View class="flex-1 gap-2 p-4">
      <Text>{state.device ? `device: ${state.device.label}` : "device: pending"}</Text>
      <Text>{ref.current ? `canvas: ${ref.current.getContextId()}` : "canvas: none"}</Text>
      <Canvas ref={ref} style={{ flex: 1 }} />
    </View>
  );
}

export function SurfaceScreen() {
  const hooked = useSurface();
  return (
    <View class="flex-1">
      <Text>{hooked.surface ? `surface: ${hooked.surface.width}x${hooked.surface.height}` : "surface: none"}</Text>
      <Canvas ref={hooked.ref} style={{ flex: 1 }} />
    </View>
  );
}

function Scene() {
  const { device } = useMainDevice();
  return <Text>{`main device: ${device ? device.label : "null"}`}</Text>;
}

export function ProviderScreen() {
  return (
    <GPUDeviceProvider deviceDescriptor={{ label: "provided" }}>
      <Scene />
    </GPUDeviceProvider>
  );
}
