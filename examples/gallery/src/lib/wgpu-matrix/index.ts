// The part of wgpu-matrix 3.x the ported demos use (see LICENSE): NSH's
// bundler resolves no npm packages, so the functions are written here with
// wgpu-matrix's names, argument order, optional `dst` and conventions
// (column-major Float32Array, mat3 padded to 12 floats, perspective depth in
// [0, 1], `lookAt` a view matrix).

export type Mat3 = Float32Array;
export type Mat4 = Float32Array;
export type Vec2 = Float32Array;
export type Vec3 = Float32Array;
export type Vec4 = Float32Array;
/** What the functions read: any array of numbers (wgpu-matrix's `BaseArgType`). */
type Arg = ArrayLike<number>;
/** What they may write to: a `dst` (wgpu-matrix accepts number arrays too). */
type Dst = Float32Array | number[];

const out = <T extends Dst>(n: number, dst?: T): T => (dst ?? new Float32Array(n)) as T;

export const vec2 = {
  create: (x = 0, y = 0): Vec2 => new Float32Array([x, y]),
  fromValues: (x = 0, y = 0): Vec2 => new Float32Array([x, y]),
};

export const vec3 = {
  create: (x = 0, y = 0, z = 0): Vec3 => new Float32Array([x, y, z]),
  fromValues: (x = 0, y = 0, z = 0): Vec3 => new Float32Array([x, y, z]),
  copy<T extends Dst = Vec3>(v: Arg, dst?: T): T {
    const d = out(3, dst);
    d[0] = v[0];
    d[1] = v[1];
    d[2] = v[2];
    return d;
  },
  add<T extends Dst = Vec3>(a: Arg, b: Arg, dst?: T): T {
    const d = out(3, dst);
    d[0] = a[0] + b[0];
    d[1] = a[1] + b[1];
    d[2] = a[2] + b[2];
    return d;
  },
  subtract<T extends Dst = Vec3>(a: Arg, b: Arg, dst?: T): T {
    const d = out(3, dst);
    d[0] = a[0] - b[0];
    d[1] = a[1] - b[1];
    d[2] = a[2] - b[2];
    return d;
  },
  sub<T extends Dst = Vec3>(a: Arg, b: Arg, dst?: T): T {
    return vec3.subtract(a, b, dst);
  },
  dot: (a: Arg, b: Arg): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2],
  cross<T extends Dst = Vec3>(a: Arg, b: Arg, dst?: T): T {
    const d = out(3, dst);
    const t0 = a[1] * b[2] - a[2] * b[1];
    const t1 = a[2] * b[0] - a[0] * b[2];
    const t2 = a[0] * b[1] - a[1] * b[0];
    d[0] = t0;
    d[1] = t1;
    d[2] = t2;
    return d;
  },
  /** `v` (w = 1) transformed by `m`, divided by w. */
  transformMat4<T extends Dst = Vec3>(v: Arg, m: Arg, dst?: T): T {
    const d = out(3, dst);
    const x = v[0];
    const y = v[1];
    const z = v[2];
    const w = m[3] * x + m[7] * y + m[11] * z + m[15] || 1;
    d[0] = (m[0] * x + m[4] * y + m[8] * z + m[12]) / w;
    d[1] = (m[1] * x + m[5] * y + m[9] * z + m[13]) / w;
    d[2] = (m[2] * x + m[6] * y + m[10] * z + m[14]) / w;
    return d;
  },
  normalize<T extends Dst = Vec3>(v: Arg, dst?: T): T {
    const d = out(3, dst);
    const len = Math.sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    if (len > 0.00001) {
      d[0] = v[0] / len;
      d[1] = v[1] / len;
      d[2] = v[2] / len;
    } else {
      d[0] = 0;
      d[1] = 0;
      d[2] = 0;
    }
    return d;
  },
};

export const vec4 = {
  create: (x = 0, y = 0, z = 0, w = 0): Vec4 => new Float32Array([x, y, z, w]),
  fromValues: (x = 0, y = 0, z = 0, w = 0): Vec4 => new Float32Array([x, y, z, w]),
};

export const mat3 = {
  /** The upper-left 3x3 of `m`, as wgpu-matrix's padded mat3 (three columns of four floats). */
  fromMat4<T extends Dst = Mat3>(m: Arg, dst?: T): T {
    const d = out(12, dst);
    d[0] = m[0];
    d[1] = m[1];
    d[2] = m[2];
    d[3] = 0;
    d[4] = m[4];
    d[5] = m[5];
    d[6] = m[6];
    d[7] = 0;
    d[8] = m[8];
    d[9] = m[9];
    d[10] = m[10];
    d[11] = 0;
    return d;
  },
};

function set<T extends Dst>(d: T, values: number[]): T {
  for (let i = 0; i < 16; i++) d[i] = values[i];
  return d;
}

export const mat4 = {
  create(...values: number[]): Mat4 {
    const d = new Float32Array(16);
    for (let i = 0; i < values.length && i < 16; i++) d[i] = values[i];
    return d;
  },
  identity: <T extends Dst = Mat4>(dst?: T): T => set(out(16, dst), [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]),
  copy<T extends Dst = Mat4>(m: Arg, dst?: T): T {
    const d = out(16, dst);
    for (let i = 0; i < 16; i++) d[i] = m[i];
    return d;
  },
  /** `a * b`. */
  multiply<T extends Dst = Mat4>(a: Arg, b: Arg, dst?: T): T {
    const r: number[] = new Array(16);
    for (let c = 0; c < 4; c++) {
      for (let row = 0; row < 4; row++) {
        let sum = 0;
        for (let k = 0; k < 4; k++) sum += a[k * 4 + row] * b[c * 4 + k];
        r[c * 4 + row] = sum;
      }
    }
    return set(out(16, dst), r);
  },
  mul<T extends Dst = Mat4>(a: Arg, b: Arg, dst?: T): T {
    return mat4.multiply(a, b, dst);
  },
  transpose<T extends Dst = Mat4>(m: Arg, dst?: T): T {
    const r: number[] = new Array(16);
    for (let c = 0; c < 4; c++) for (let row = 0; row < 4; row++) r[c * 4 + row] = m[row * 4 + c];
    return set(out(16, dst), r);
  },
  inverse<T extends Dst = Mat4>(m: Arg, dst?: T): T {
    const [a00, a01, a02, a03, a10, a11, a12, a13, a20, a21, a22, a23, a30, a31, a32, a33] = Array.from(
      { length: 16 },
      (_, i) => m[i],
    );
    const b00 = a00 * a11 - a01 * a10;
    const b01 = a00 * a12 - a02 * a10;
    const b02 = a00 * a13 - a03 * a10;
    const b03 = a01 * a12 - a02 * a11;
    const b04 = a01 * a13 - a03 * a11;
    const b05 = a02 * a13 - a03 * a12;
    const b06 = a20 * a31 - a21 * a30;
    const b07 = a20 * a32 - a22 * a30;
    const b08 = a20 * a33 - a23 * a30;
    const b09 = a21 * a32 - a22 * a31;
    const b10 = a21 * a33 - a23 * a31;
    const b11 = a22 * a33 - a23 * a32;
    const det = 1 / (b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06);
    return set(out(16, dst), [
      (a11 * b11 - a12 * b10 + a13 * b09) * det,
      (a02 * b10 - a01 * b11 - a03 * b09) * det,
      (a31 * b05 - a32 * b04 + a33 * b03) * det,
      (a22 * b04 - a21 * b05 - a23 * b03) * det,
      (a12 * b08 - a10 * b11 - a13 * b07) * det,
      (a00 * b11 - a02 * b08 + a03 * b07) * det,
      (a32 * b02 - a30 * b05 - a33 * b01) * det,
      (a20 * b05 - a22 * b02 + a23 * b01) * det,
      (a10 * b10 - a11 * b08 + a13 * b06) * det,
      (a01 * b08 - a00 * b10 - a03 * b06) * det,
      (a30 * b04 - a31 * b02 + a33 * b00) * det,
      (a21 * b02 - a20 * b04 - a23 * b00) * det,
      (a11 * b07 - a10 * b09 - a12 * b06) * det,
      (a00 * b09 - a01 * b07 + a02 * b06) * det,
      (a31 * b01 - a30 * b03 - a32 * b00) * det,
      (a20 * b03 - a21 * b01 + a22 * b00) * det,
    ]);
  },
  invert<T extends Dst = Mat4>(m: Arg, dst?: T): T {
    return mat4.inverse(m, dst);
  },
  /** A perspective projection with depth in [0, 1]; `zFar` Infinity for an infinite far plane. */
  perspective<T extends Dst = Mat4>(fieldOfViewYInRadians: number, aspect: number, zNear: number, zFar: number, dst?: T): T {
    const f = Math.tan(Math.PI * 0.5 - 0.5 * fieldOfViewYInRadians);
    const r = [f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, 0, -1, 0, 0, 0, 0];
    if (Number.isFinite(zFar)) {
      const rangeInv = 1 / (zNear - zFar);
      r[10] = zFar * rangeInv;
      r[14] = zFar * zNear * rangeInv;
    } else {
      r[10] = -1;
      r[14] = -zNear;
    }
    return set(out(16, dst), r);
  },
  ortho<T extends Dst = Mat4>(left: number, right: number, bottom: number, top: number, near: number, far: number, dst?: T): T {
    return set(out(16, dst), [
      2 / (right - left),
      0,
      0,
      0,
      0,
      2 / (top - bottom),
      0,
      0,
      0,
      0,
      1 / (near - far),
      0,
      (right + left) / (left - right),
      (top + bottom) / (bottom - top),
      near / (near - far),
      1,
    ]);
  },
  /** The view matrix of a camera at `eye` looking at `target`. */
  lookAt<T extends Dst = Mat4>(eye: Arg, target: Arg, up: Arg, dst?: T): T {
    const z = vec3.normalize(vec3.subtract(eye, target));
    const x = vec3.normalize(vec3.cross(up, z));
    const y = vec3.normalize(vec3.cross(z, x));
    return set(out(16, dst), [
      x[0],
      y[0],
      z[0],
      0,
      x[1],
      y[1],
      z[1],
      0,
      x[2],
      y[2],
      z[2],
      0,
      -vec3.dot(x, eye),
      -vec3.dot(y, eye),
      -vec3.dot(z, eye),
      1,
    ]);
  },
  translation: <T extends Dst = Mat4>(v: Arg, dst?: T): T =>
    set(out(16, dst), [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, v[0], v[1], v[2], 1]),
  scaling: <T extends Dst = Mat4>(v: Arg, dst?: T): T => set(out(16, dst), [v[0], 0, 0, 0, 0, v[1], 0, 0, 0, 0, v[2], 0, 0, 0, 0, 1]),
  rotationX<T extends Dst = Mat4>(angle: number, dst?: T): T {
    const c = Math.cos(angle);
    const s = Math.sin(angle);
    return set(out(16, dst), [1, 0, 0, 0, 0, c, s, 0, 0, -s, c, 0, 0, 0, 0, 1]);
  },
  rotationY<T extends Dst = Mat4>(angle: number, dst?: T): T {
    const c = Math.cos(angle);
    const s = Math.sin(angle);
    return set(out(16, dst), [c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, 0, 0, 0, 1]);
  },
  rotationZ<T extends Dst = Mat4>(angle: number, dst?: T): T {
    const c = Math.cos(angle);
    const s = Math.sin(angle);
    return set(out(16, dst), [c, s, 0, 0, -s, c, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
  },
  axisRotation<T extends Dst = Mat4>(axis: Arg, angle: number, dst?: T): T {
    const [x, y, z] = vec3.normalize(axis);
    const xx = x * x;
    const yy = y * y;
    const zz = z * z;
    const c = Math.cos(angle);
    const s = Math.sin(angle);
    const o = 1 - c;
    return set(out(16, dst), [
      xx + (1 - xx) * c,
      x * y * o + z * s,
      x * z * o - y * s,
      0,
      x * y * o - z * s,
      yy + (1 - yy) * c,
      y * z * o + x * s,
      0,
      x * z * o + y * s,
      y * z * o - x * s,
      zz + (1 - zz) * c,
      0,
      0,
      0,
      0,
      1,
    ]);
  },
  rotation<T extends Dst = Mat4>(axis: Arg, angle: number, dst?: T): T {
    return mat4.axisRotation(axis, angle, dst);
  },
  translate: <T extends Dst = Mat4>(m: Arg, v: Arg, dst?: T): T => mat4.multiply(m, mat4.translation(v), dst),
  scale: <T extends Dst = Mat4>(m: Arg, v: Arg, dst?: T): T => mat4.multiply(m, mat4.scaling(v), dst),
  rotateX: <T extends Dst = Mat4>(m: Arg, angle: number, dst?: T): T => mat4.multiply(m, mat4.rotationX(angle), dst),
  rotateY: <T extends Dst = Mat4>(m: Arg, angle: number, dst?: T): T => mat4.multiply(m, mat4.rotationY(angle), dst),
  rotateZ: <T extends Dst = Mat4>(m: Arg, angle: number, dst?: T): T => mat4.multiply(m, mat4.rotationZ(angle), dst),
  rotate: <T extends Dst = Mat4>(m: Arg, axis: Arg, angle: number, dst?: T): T =>
    mat4.multiply(m, mat4.axisRotation(axis, angle), dst),
};
