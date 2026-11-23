// The flag constants, with the spec's values (wgpu's bits are the same).

/** `GPUBufferUsage`. */
export const GPUBufferUsage = Object.freeze({
  MAP_READ: 0x0001,
  MAP_WRITE: 0x0002,
  COPY_SRC: 0x0004,
  COPY_DST: 0x0008,
  INDEX: 0x0010,
  VERTEX: 0x0020,
  UNIFORM: 0x0040,
  STORAGE: 0x0080,
  INDIRECT: 0x0100,
  QUERY_RESOLVE: 0x0200,
} as const);

/** `GPUTextureUsage`. */
export const GPUTextureUsage = Object.freeze({
  COPY_SRC: 0x01,
  COPY_DST: 0x02,
  TEXTURE_BINDING: 0x04,
  STORAGE_BINDING: 0x08,
  RENDER_ATTACHMENT: 0x10,
} as const);

/** `GPUShaderStage`. */
export const GPUShaderStage = Object.freeze({
  VERTEX: 0x1,
  FRAGMENT: 0x2,
  COMPUTE: 0x4,
} as const);

/** `GPUMapMode`. */
export const GPUMapMode = Object.freeze({
  READ: 0x0001,
  WRITE: 0x0002,
} as const);

/** `GPUColorWrite`. */
export const GPUColorWrite = Object.freeze({
  RED: 0x1,
  GREEN: 0x2,
  BLUE: 0x4,
  ALPHA: 0x8,
  ALL: 0xf,
} as const);
