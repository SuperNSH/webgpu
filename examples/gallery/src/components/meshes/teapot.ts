// STAND-IN for apps/example/src/components/meshes/teapot.ts, which imports
// the npm package `teapot` (the Utah teapot's positions and cells). NSH's
// bundler resolves no npm packages and the package is not vendored here
// (fetching it needs the user's go-ahead), so this module gives the Stanford
// dragon the example already ships (stanfordDragonData.ts), centered and
// scaled to the teapot's extent (about 40 units across), with the same
// shape of data and the same normals computation. ABuffer and Wireframe
// draw it where the reference draws the teapot (examples/gallery/KNOWN_ISSUES.md).

import dragonRawData from "./stanfordDragonData";
import { computeSurfaceNormals } from "./utils";

const raw = dragonRawData.positions as [number, number, number][];
const min = [0, 1, 2].map((i) => Math.min(...raw.map((p) => p[i])));
const max = [0, 1, 2].map((i) => Math.max(...raw.map((p) => p[i])));
const center = [0, 1, 2].map((i) => (min[i] + max[i]) / 2);
const scale = 40 / Math.max(...[0, 1, 2].map((i) => max[i] - min[i]));

export const mesh = {
  positions: raw.map((p) => p.map((v, i) => (v - center[i]) * scale) as [number, number, number]),
  triangles: dragonRawData.cells as [number, number, number][],
  normals: [] as [number, number, number][],
};

// Compute surface normals
mesh.normals = computeSurfaceNormals(mesh.positions, mesh.triangles);
