# NSH API

react-native-webgpu's React API (`useDevice`, `useCanvasRef`, `useSurface`, `GPUDeviceProvider`, `useMainDevice`, `warnIfNotHardwareAccelerated`), on NSH.

NSH's TSX has JSX and signals, not React. A component runs once; only the bindings that read a changed signal run again. Each hook keeps the reference's name, arguments and return shape, and holds in a signal what React would re-render with. The objects the hooks return have **getters that read signals**: read `state.device` in JSX or in an `effect` and it updates. Destructuring in a component body (`const { device } = useDevice()`) takes the value at that moment, as any signal read there does. The exception is `useMainDevice()` inside a `<GPUDeviceProvider>`, whose children mount only once the device exists.

## Mapping

| React (reference) | NSH (nsh-webgpu) |
|---|---|
| `useDevice(adapterOptions?, deviceDescriptor?)` → `{ adapter, device }` after a re-render | Starts the requests when called; returns `{ get adapter(), get device() }`, both `null` until ready. Same errors, thrown in the request's promise: `No appropriate GPUAdapter found.`, `No appropriate GPUDevice found.` Never destroys the device (nor does the reference). |
| `useCanvasRef()` = `useRef<CanvasRef>(null)` | A function you pass as `<Canvas ref={ref}>`, with a `current` property backed by a signal. Type `CanvasRefObject`. |
| `useSurface()` → `{ ref, surface }` from a layout effect | `ref` is a `useCanvasRef()`; `surface` reads `getNativeSurface()` once the canvas mounted, again on each new layout or when its surface attaches, `null` before mount and after unmount. The reference crashes with no canvas; here `surface` stays `null`. |
| `GPUDeviceProvider` (React context, renders `null` until ready) | An NSH `context()`. The provider calls `useDevice` and renders its children inside the context once the device exists. Props: `children`, `adapterOptions`, `deviceDescriptor`. |
| `useMainDevice()` (`useContext`) | Reads the context; throws `No DeviceContext found.` outside a provider. |
| `warnIfNotHardwareAccelerated(adapter)` | The same function and text: `console.warn` when `adapter.info.isFallbackAdapter`. |
| `useNativeBufferSize(debounceMs = 150)` | Same behaviour, `size` as a getter. Useful only with a producer of pixel sizes: `NativeBufferView` is unsupported. |

## useDevice

```tsx
import { Text, View } from "nsh";
import { Canvas, useCanvasRef, useDevice } from "nsh-webgpu";

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
```

(`examples/headless/samples/hooks.tsx`, type-checked; the same calls run in `examples/headless/react-parity.app.tsx`, scenario `device`, which `react-parity.test.tsx` checks.) To draw once the device is there, read `state.device` in an `effect`, or use the provider.

## useCanvasRef

```tsx
const ref = useCanvasRef();
// <Canvas ref={ref} style={{ flex: 1 }} />
const context = ref.current?.getContext("webgpu");
```

`ref.current` is `null` until the canvas is mounted and laid out, and `null` again after it unmounts; it is a signal, so a binding that reads it runs again then. A plain callback works as well (`ref={(canvas) => canvas && draw(canvas)}`), as the README's example shows: it is called with the canvas, then with `null` on unmount.

## useSurface

```tsx
const hooked = useSurface();
// <Canvas ref={hooked.ref} style={{ flex: 1 }} />
// hooked.surface: { surface, width, height, clientWidth, clientHeight } or null
```

`hooked.surface` is `null` until the canvas mounted, then `getNativeSurface()`, read again when the canvas lays out anew (a resize) or its view's surface attaches, and `null` after it unmounts (the reference reads it once, in a layout effect). `surface.surface` is the native handle as a `bigint` (the `CAMetalLayer *` on Apple platforms), `0n` before the view attached; `width`/`height` are the view's size in points (the layout's until the view reports one). Tested with the real `<Canvas>` in `examples/headless/canvas-lifecycle.test.tsx`. nsh-webgpu has no API that takes it back (see [Platform guide](native-api.md#rendering-to-a-native-window)).

## GPUDeviceProvider and useMainDevice

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

(`samples/hooks.tsx`; tested as `react-parity.app.tsx` scenario `provider`: two consumers share one device, and `useMainDevice()` outside throws.) The provider renders nothing until the device is ready. To show something meanwhile, render it beside the provider, as the reference's docs say.

## warnIfNotHardwareAccelerated

See [its page](../api/warn-if-not-hardware-accelerated.md). `useDevice` already calls it.

## See also

- [API: GPUDeviceProvider](../api/gpu-device-provider.md)
- `docs/parity/react-parity.md`: the mapping with its tests
