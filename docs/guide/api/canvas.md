# Canvas

`<Canvas>` renders an outer `View` with the props given, around the native view `webgpu.canvas` (`flex: 1`), and gives its ref a `CanvasRef`. It allocates a context id when it mounts and retires it when it unmounts. As in the reference, a context made before keeps working: it keeps its configuration and renders offscreen once the view went, so a render loop that outlives its `<Canvas>` does not throw.

## Props

`CanvasProps` extends NSH's `ViewProps` (without `ref` and `android`, which it redefines). `children` are accepted and not rendered, as in the reference, whose native view replaces them. `NativeProps` (the native view's props, the reference's `WebGPUViewNativeComponent` type) is exported too.

| Prop | Type | Default | |
|---|---|---|---|
| `ref` | `(canvas: CanvasRef \| null) => void`, a variable, or `{ current: CanvasRef \| null }` | | Receives the `CanvasRef` once the canvas is mounted (after its view's first layout), and `null` when it unmounts, as the reference's ref. A callback, a variable (`ref={canvas}`, which NSH compiles to an assignment), a [`useCanvasRef()`](../getting-started/nsh-api.md#usecanvasref), or an object with `current`. |
| `colorSpace` | `"srgb" \| "display-p3"` | `"srgb"` | The primaries of the canvas's values. Tags the layer on macOS and iOS; read at mount. Ignored on Android. |
| `toneMapping` | `GPUCanvasToneMapping` | `{ mode: "standard" }` | For `rgba16float` canvases; read at mount. See [Canvas, HDR](../getting-started/canvas.md#hdr). |
| `android` | `AndroidCanvasProps` | `{ backingView: "TextureView" }` | Android only, below. |
| `transparent` | `boolean` | | react-native-wgpu's former prop. On Android it asks for a `TextureView`; elsewhere `alphaMode` decides. Not in the reference at `06599f9`. |
| any `ViewProps` | | | Applied to the outer `View` (`style`, `class`, `onLayout`, `testID`, ...). |

Changing `colorSpace`, `toneMapping` or `android.backingView` on a mounted canvas has no effect, as in the reference, and warns in development builds (`appInfo().development`, the reference's `__DEV__`). On Android it also warns, as the reference does, for `HardwareBufferView` below Android 10 and for a `zOrderOnTop`/`zOrderMediaOverlay` change on a `SurfaceView` below Android 11. Remount the canvas with a new `key` instead.

## CanvasRef

| Member | |
|---|---|
| `getContext("webgpu")` | A new `RNCanvasContext` (`GPUCanvasContext` plus `present()`) on each call, all sharing the canvas's swap chain. Its canvas is the view's layout size in points. Any other name throws `[WebGPU] Unsupported context: ...`; on a canvas that unmounted it throws `[WebGPU] Cannot get context before mount`. |
| `getContextId()` | The canvas's context id. |
| `getNativeSurface()` | `{ surface, width, height, clientWidth, clientHeight }` plus no-op DOM methods. `surface` is the native handle as a `bigint` (`CAMetalLayer *` on Apple platforms), `0n` before the view attached. |

## GPUCanvasContext

`configure`, `unconfigure`, `getConfiguration`, `getCurrentTexture` per spec, plus `present()`:

- `configure`: `format` is `bgra8unorm`, `rgba8unorm` or `rgba16float` (anything else is a `TypeError`); `usage` defaults to `RENDER_ATTACHMENT`; `viewFormats`, `alphaMode`, `colorSpace`, `toneMapping`. Presentation is always FIFO (vsync), as in the reference.
- `getConfiguration()` returns the configuration with its defaults filled in. The reference has none.
- `getCurrentTexture()` returns the same `GPUTexture` until `present()`, `configure()`, `unconfigure()` or a new `canvas.width`/`height`; before `configure()` it throws `InvalidStateError`.
- `unconfigure()` drops the configuration; wgpu has no surface unconfigure, so the surface itself stays configured.

All tested headless in `examples/headless/surfaces-apple.test.tsx`; the swap chain on a real `CAMetalLayer` in `tests/surfaces_apple/`.

## Usage

[README, "Example"](../../../README.md#example) and [Getting started: Canvas](../getting-started/canvas.md).

## Android rendering options

```ts
type AndroidCanvasProps =
  | { backingView?: "TextureView"; opaque?: boolean } // opaque defaults to false
  | { backingView: "SurfaceView"; opaque?: boolean; zOrderOnTop?: boolean; zOrderMediaOverlay?: boolean } // opaque defaults to true
  | { backingView: "HardwareBufferView" };
```

- `TextureView` (default): composited like any view, transparent unless `opaque: true`.
- `SurfaceView`: its own compositor layer. `zOrderOnTop` puts it above every view of the window, `zOrderMediaOverlay` above other `SurfaceView`s. Opacity and z-order apply live.
- `HardwareBufferView`: falls back to a `TextureView`, with a warning. The reference draws each frame's `AHardwareBuffer` through a Java `HardwareRenderer`; an nshm ships no Java.

Both `TextureView` and `SurfaceView` were seen on screen on an emulator with the merged code (`docs/evidence/integration/android-canvas-native.png`), and `<Canvas>` itself in `examples/canvas-apple` (`docs/evidence/fixes/android-canvas-apple.png`). The view follows its surface on the UI thread's `AChoreographer` each vsync rather than a `SurfaceHolder.Callback` (no Java), so a surface the system destroys is detached at the next vsync.

## Other platforms

- Windows: a child `HWND`; it sits above anything NSH draws over the canvas. Compile-checked only.
- Linux: a `gtk::Picture` showing offscreen frames, one GPU→CPU→GPU copy per presented frame (GTK 4 gives a widget no window of its own, and wgpu cannot export a dmabuf). Compile-checked only.
- Web: the browser's own WebGPU and `<canvas>` (id `rnwgpu-canvas-<id>`), with a no-op `present()` and a drawing buffer of the layout size times `devicePixelRatio`, as the reference's web build.

`docs/parity/surfaces-apple.md` and `docs/parity/surfaces-other.md` have every row.
