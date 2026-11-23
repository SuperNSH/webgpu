# drawElementImageToTexture

Not exported.

In react-native-webgpu, `await device.queue.drawElementImageToTexture({ source: viewRef }, { texture })` rasterizes a mounted native view on the UI thread and writes its pixels into a `GPUTexture`, following the html-in-canvas proposal. nsh-webgpu has no `GPUQueue.drawElementImageToTexture` method: calling it fails with `drawElementImageToTexture is not a function`. Only its argument types are exported (`GPUDrawElementImageSourceView`, `GPUDrawElementImageSource`, `GPUDrawElementImageDestination`), with the source view typed as `object` instead of `React.Component`.

Two pieces are missing for it:

- A way to get a view's pixels from NSH. NSH's test runner can screenshot an app (`app.screenshot()`), but there is no API that renders one view into a buffer for app code. The canvas area found that `app.screenshot()` returns a black picture on a Mac without Screen Recording permission.
- The method itself on `GPUQueue`, throwing `ERR_WEBGPU_UNSUPPORTED` as the project's rule asks of every export it cannot back.

Until then, render what you need into a texture with WebGPU itself, or upload pixels you already have with `queue.writeTexture()` or `copyExternalImageToTexture()`.
