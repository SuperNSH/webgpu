# GPUDeviceProvider

`<GPUDeviceProvider>` requests one adapter and device with [`useDevice`](../getting-started/nsh-api.md#usedevice) and shares them with the views inside. It renders nothing until the device is ready, then its children inside an NSH `context()`.

## Props

`DeviceProviderProps`, as the reference's:

| Prop | Type | |
|---|---|---|
| `children` | `JSX.Element` | What renders once the device is ready. |
| `adapterOptions` | `GPURequestAdapterOptions` | Passed to `navigator.gpu.requestAdapter()`. |
| `deviceDescriptor` | `GPUDeviceDescriptor` | Passed to `adapter.requestDevice()`. |

## Usage

```tsx
import { Text } from "nsh";
import { GPUDeviceProvider, useMainDevice } from "nsh-webgpu";

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
```

(`examples/headless/samples/hooks.tsx`, type-checked; `examples/headless/react-parity.app.tsx`, scenario `provider`, is tested.) Destructuring `useMainDevice()` in a component body is safe here: the children mount only once the device exists.

To show something while the device loads, render it beside the provider; the provider has no loading prop, as in the reference.

## useMainDevice

`useMainDevice()` returns `{ device, adapter }` of the nearest provider. Outside any provider it throws `No DeviceContext found.` (tested with an `ErrorBoundary` in `react-parity.app.tsx`).

## See also

- [NSH API](../getting-started/nsh-api.md)
- [warnIfNotHardwareAccelerated](warn-if-not-hardware-accelerated.md)
