# Learn WebGPU

nsh-webgpu is the WebGPU API, so material written for the browser applies. Where NSH differs (presenting with `context.present()`, sizing the canvas with `pixelRatio()`, no DOM), the [Canvas](canvas.md) and [Platform guide](native-api.md) pages say so.

- [WebGPU Fundamentals](https://webgpufundamentals.org/) teaches the API from the ground up.
- The [WebGPU specification](https://www.w3.org/TR/webgpu/) and the [WGSL specification](https://www.w3.org/TR/WGSL/) are the references nsh-webgpu follows; `tsx/types/webgpu.d.ts` is the typed form of the API it implements.
- [wgpu](https://wgpu.rs) is the library underneath. Its WGSL compiler, naga, reports the shader errors you see in `getCompilationInfo()` and in validation errors.
- The reference's examples (`apps/example/src` in react-native-webgpu) port with few changes; `examples/canvas-apple/scenes.ts` is its Triangle and Cube, and `examples/gallery` collects more.
