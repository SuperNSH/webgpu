# Third-party notices

nsh-webgpu is MIT-licensed (© 2026 hmziqrs, see `LICENSE`). It contains material from the projects below, each
under its own licence, reproduced here as the licences require.

## react-native-webgpu (MIT)

Source: https://github.com/wcandillon/react-native-webgpu, commit 06599f9.

What derives from it: the TSX package's public API and the code ported from `packages/webgpu/src` (`tsx/Canvas.tsx`,
`tsx/hooks.tsx`, `tsx/GPUDeviceProvider.tsx`, `tsx/Offscreen.ts`, `tsx/formats.ts`, `tsx/video.ts`,
`tsx/NativeBufferView.tsx`, `tsx/importDevice.ts`, `tsx/animationFrame.ts`, `tsx/types.ts`, the doc comments
and error messages copied from it), the parity tests that follow its tests, and the example gallery
(`examples/gallery/`), ported from its `apps/example` (see `examples/gallery/LICENSES.md`).

`packages/webgpu/LICENSE`, verbatim:

```text
MIT License

Copyright (c) 2024 William Candillon
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

The repository root's `LICENSE.md` (which covers `apps/example`), verbatim:

```text
MIT License

Copyright 2024-present, William Candillon.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## webgpu-samples (BSD 3-Clause)

Source: https://github.com/webgpu/webgpu-samples, licence: BSD 3-Clause
(https://github.com/webgpu/webgpu-samples/blob/main/LICENSE.txt).

What derives from it: several of the gallery's demos and their WGSL shaders, which react-native-webgpu's
`apps/example` adapted from the WebGPU samples; `examples/gallery/LICENSES.md` lists them. Only
`examples/gallery/src/demos/ImageBlur/Shaders.ts` names the sample in the reference's own header; the others are
matched by the sample's name and code.

The licence text below is webgpu-samples' `LICENSE.txt`, verbatim:

```text
Copyright 2019 WebGPU Samples Contributors

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

   1. Redistributions of source code must retain the above copyright notice,
      this list of conditions and the following disclaimer.

   2. Redistributions in binary form must reproduce the above copyright notice,
      this list of conditions and the following disclaimer in the documentation
      and/or other materials provided with the distribution.

   3. Neither the name of the copyright holder nor the names of its
      contributors may be used to endorse or promote products derived from this
      software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## @webgpu/types (BSD 3-Clause)

Source: https://github.com/gpuweb/types, version 0.1.74. `tsx/types/webgpu.d.ts` is that package's declarations,
vendored unchanged; its header carries the licence, reproduced here:

```text
Copyright 2022 WebGPU Developers

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

   1. Redistributions of source code must retain the above copyright notice,
      this list of conditions and the following disclaimer.

   2. Redistributions in binary form must reproduce the above copyright notice,
      this list of conditions and the following disclaimer in the documentation
      and/or other materials provided with the distribution.

   3. Neither the name of the copyright holder nor the names of its
      contributors may be used to endorse or promote products derived from this
      software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## wgpu-matrix (MIT)

`examples/gallery/src/lib/wgpu-matrix/` re-implements the part of wgpu-matrix 3.x (© 2022 Gregg Tavares) that the
demos call, with its names and conventions. Its MIT licence is in `examples/gallery/src/lib/wgpu-matrix/LICENSE`.

## Gallery assets

- `examples/gallery/assets/cubemap/`: Emil Persson (Humus), Creative Commons Attribution 3.0 Unported; terms in
  `examples/gallery/assets/cubemap/readme.txt` and `credit.txt`, which ship with the images.
- The other images in `examples/gallery/assets/`: from react-native-webgpu's `apps/example/src/assets`, under its MIT
  licence (above; also `examples/gallery/assets/LICENSE-react-native-webgpu.md`).
- The Stanford dragon mesh (`examples/gallery/src/components/meshes/stanfordDragonData.ts`, also read by
  `teapot.ts`): copied from react-native-webgpu's `apps/example/src/components/meshes/stanfordDragonData.ts`, whose
  only provenance note is its header line `// from github.com/hughsk/stanford-dragon`. The scan itself is from the
  Stanford 3D Scanning Repository (Stanford University Computer Graphics Laboratory,
  https://graphics.stanford.edu/data/3Dscanrep/), which asks users of its models to acknowledge it as the source:
  this entry is that acknowledgement. The `stanford-dragon` package (https://github.com/hughsk/stanford-dragon)
  has no licence file and no `license` field in its `package.json` (checked 2026-11-24), so no further terms could be
  established. The mesh is redistributed here only as demo data in `examples/gallery`, as react-native-webgpu and
  webgpu-samples redistribute it; it is not part of the `nsh-webgpu` crate.
