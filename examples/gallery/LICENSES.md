# Gallery licences

The gallery is a port of react-native-webgpu's example app (`apps/example`, commit 06599f9). Every file under
`src/` and `assets/` derives from it unless listed otherwise, and is under react-native-webgpu's MIT licence
(`assets/LICENSE-react-native-webgpu.md`; both of its licence files are reproduced in the repository's
[NOTICE.md](../../NOTICE.md)). Groups with further terms:

| Files | Origin | Licence |
|---|---|---|
| `src/demos/Triangle/`, `src/demos/Cube/` (rotating, textured, instanced, fractal cube, cubemap), `src/components/cube.ts`, `src/demos/Sampler/`, `src/demos/ReversedZ/`, `src/demos/RenderBundles/`, `src/demos/OcclusionQuery/`, `src/demos/ComputeBoids/`, `src/demos/Particles/`, `src/demos/ImageBlur/`, `src/demos/ShadowMapping/` (shadow mapping, deferred rendering), `src/demos/ABuffer/`, `src/demos/Wireframe/`, `src/components/meshes/{mesh,sphere,utils}.ts` | [webgpu-samples](https://github.com/webgpu/webgpu-samples) samples of the same names (`helloTriangle`, `helloTriangleMSAA`, `rotatingCube`, `texturedCube`, `instancedCube`, `fractalCube`, `cubemap`, `samplerParameters`, `reversedZ`, `renderBundles`, `occlusionQuery`, `computeBoids`, `particles`, `imageBlur`, `shadowMapping`, `deferredRendering`, `a-buffer`, `wireframe`), adapted by react-native-webgpu. Only `ImageBlur/Shaders.ts` says so in a header; the rest are matched by name and code. | BSD 3-Clause (webgpu-samples), then MIT (react-native-webgpu). Licence text: NOTICE.md (to be pasted before publishing) |
| `src/components/meshes/stanfordDragonData.ts`, `stanfordDragon.ts`, `teapot.ts` (a stand-in drawn from the dragon data) | The Stanford dragon, via `github.com/hughsk/stanford-dragon` (the reference file's header) | **Not established**: neither the package's licence nor the Stanford 3D Scanning Repository's terms are in the reference checkout. See NOTICE.md |
| `src/demos/ComputeToys/` | react-native-webgpu's port of the compute-toys project (its header: "TypeScript port of the Rust compute-toys project") | MIT (react-native-webgpu); compute-toys' own licence is not in the reference checkout and is not established here |
| `src/demos/MNISTInference/` | react-native-webgpu's TypeGPU demo; the weights are downloaded at run time from TypeGPU's site, not shipped | MIT (react-native-webgpu) |
| `src/lib/wgpu-matrix/` | A re-implementation of wgpu-matrix 3.x's API | wgpu-matrix's MIT licence: `src/lib/wgpu-matrix/LICENSE` |
| `assets/cubemap/` | Emil Persson (Humus) | CC BY 3.0, `assets/cubemap/readme.txt` and `credit.txt` |
| `assets/*.png` | react-native-webgpu `apps/example/src/assets` | MIT (react-native-webgpu), `assets/README.md` |
| `src/harness/`, `src/ui/`, `*.test.tsx`, `App.tsx`, `native/` | Written for this port | MIT (nsh-webgpu, `../../LICENSE`) |
