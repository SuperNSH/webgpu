# Canvas

`<Canvas>` is a view the GPU draws into. It renders an outer `View` with the props you give it, around the native view `webgpu.canvas`, which each platform registers: a `CAMetalLayer` view on macOS and iOS, a `TextureView` or `SurfaceView` on Android, a child `HWND` on Windows, a `gtk::Picture` showing offscreen frames on Linux, a `<canvas>` with the browser's WebGPU on the web. [API: Canvas](../api/canvas.md) lists the props.

## Your first triangle

The README's example is the whole app: [README, "Example"](../../../README.md#example). The steps:

1. Request an adapter and a device, and read `navigator.gpu.getPreferredCanvasFormat()`.
2. Get the canvas's `CanvasRef` from its `ref` prop. As in the reference, the canvas sets its ref once it is mounted (here: after its view's first layout) and sets it to `null` when it unmounts, so a callback ref checks for `null`. NSH has no `useRef`: a callback, a variable, or [`useCanvasRef()`](nsh-api.md#usecanvasref) holds it.
3. `getContext("webgpu")` on the ref: the view has laid out by then, so the context's canvas takes its layout size. The README's example starts drawing from the ref callback; `examples/canvas-apple/App.tsx` awaits both canvases' refs. `getContext()` on a ref whose canvas unmounted throws `[WebGPU] Cannot get context before mount`, as the reference's does.
4. `context.configure({ device, format, alphaMode })`, encode, `device.queue.submit(...)`, `context.present()`.

## Frame presentation

As in react-native-webgpu, nothing reaches the screen until you call `present()` on the context after `submit()`. `present()` is not in the WebGPU spec; on the web the browser presents for you, and nsh-webgpu's web canvas makes `present()` a no-op so the same code runs there.

```tsx
device.queue.submit([commandEncoder.finish()]);
context.present();
```

A context made before the native view attached its surface (or with no view at all, in a headless test) renders into an offscreen texture of the same format and size. When the view attaches, the last presented frame is copied onto the surface, which needs `GPUTextureUsage.COPY_DST` in the configuration's `usage`; without it wgpu reports the copy as an error and skips it.

For a render loop use [`requestAnimationFrame`](../api/request-animation-frame.md), which is NSH's frame clock.

## Pixel density

A canvas has two sizes, as on the web:

- `canvas.clientWidth` / `clientHeight`: the layout size in points. Updated from the view at each new texture.
- `canvas.width` / `height`: the drawing buffer in pixels, what WebGPU renders at. It starts at the layout size, not scaled to the screen.

For a sharp picture multiply by NSH's `pixelRatio()` (React Native's `PixelRatio.get()`):

```tsx
import { pixelRatio } from "nsh";

context.canvas.width = context.canvas.clientWidth * pixelRatio();
context.canvas.height = context.canvas.clientHeight * pixelRatio();
```

(From `examples/canvas-apple/App.tsx`.) Setting `width` or `height` reconfigures the swap chain at the next `getCurrentTexture()`. Recompute both when the layout changes.

## Transparency

Configure with `alphaMode: "premultiplied"` and clear with alpha 0 to show the views under the canvas, as the README's example does over a blue `View`:

```tsx
context.configure({ device, format, alphaMode: "premultiplied" });
// ...
colorAttachments: [{ view, clearValue: [0, 0, 0, 0], loadOp: "clear", storeOp: "store" }],
```

Per platform:

- macOS and iOS: wgpu's Metal surfaces offer only opaque and post-multiplied alpha. nsh-webgpu maps `"premultiplied"` to a non-opaque `CAMetalLayer`, which Core Animation composites as premultiplied: what the spec means. Seen on screen in the iOS simulator (`docs/evidence/surfaces-apple/ios-simulator-screen.png`).
- Android: as in the reference, the backing view decides. A `TextureView` (the default) composites with alpha unless `android={{ opaque: true }}`; a `SurfaceView` is opaque unless `opaque: false`.
- Linux: `"opaque"` frames are shown without alpha, `"premultiplied"` as premultiplied.
- Windows: with DX12 (the default) the canvas is a composition visual, so it can be transparent and NSH draws over it. Without a DX12 adapter, or with `WGPU_BACKEND` set, it is a child window: opaque, and above anything NSH draws over it.

## Color space

Pass `colorSpace: "display-p3"` to `configure()` to write Display P3 values. On macOS and iOS also pass it to the `Canvas`, which tags its layer when it mounts:

```tsx
<Canvas ref={ref} colorSpace="display-p3" style={{ flex: 1 }} />
```

The canvas reads `colorSpace` and `toneMapping` once, at mount; a later change warns and is ignored, and `configure()` warns once if it asks for another colorspace than the canvas was mounted with. Android ignores it. The Display P3 path is implemented and its layer tagging tested, but no one has looked at P3 colors on a screen. (Sample untested.)

## HDR

On an `rgba16float` canvas, `toneMapping: { mode: "extended" }` lets values above 1.0 use the display's extended range. On Apple platforms also pass it to the `Canvas`. One difference from the reference: wgpu's Metal backend turns extended dynamic range on for every `rgba16float` surface, so with `"standard"` values above 1.0 are not clipped either (`docs/parity/surfaces-apple.md`). `getPreferredHighBitDepthCanvasFormat()` gives `"rgba16float"`, or `"rgb10a2unorm"` on Android. Not checked visually.

## See also

- [API: Canvas](../api/canvas.md)
- [Platform guide](native-api.md)
- [NSH API](nsh-api.md): the hooks and the device provider
