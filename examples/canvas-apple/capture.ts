// Evidence of what reached the screen: a canvas's current texture (on a
// real platform, the CAMetalLayer's drawable) copied to a buffer before it
// is presented, encoded as a PNG and logged as base64 (`PNG <name> <b64>`),
// for when no picture of the window can be taken (no Screen Recording
// permission). The canvas must be configured with COPY_SRC.

import { deflateSync } from "zlib";

/** Copies `texture` into a mappable buffer within `encoder`; resolves to its RGBA rows. */
export function copyOut(
  device: GPUDevice,
  encoder: GPUCommandEncoder,
  texture: GPUTexture,
): () => Promise<{ width: number; height: number; rgba: Uint8Array }> {
  const { width, height } = texture;
  const bytesPerRow = Math.ceil((width * 4) / 256) * 256;
  const buffer = device.createBuffer({ size: bytesPerRow * height, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  encoder.copyTextureToBuffer({ texture }, { buffer, bytesPerRow }, [width, height]);
  const bgra = texture.format.startsWith("bgra");
  return async () => {
    await buffer.mapAsync(GPUMapMode.READ);
    const padded = new Uint8Array(buffer.getMappedRange().slice(0));
    buffer.unmap();
    const rgba = new Uint8Array(width * height * 4);
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const s = y * bytesPerRow + x * 4;
        const d = (y * width + x) * 4;
        rgba[d] = padded[s + (bgra ? 2 : 0)];
        rgba[d + 1] = padded[s + 1];
        rgba[d + 2] = padded[s + (bgra ? 0 : 2)];
        rgba[d + 3] = padded[s + 3];
      }
    }
    return { width, height, rgba };
  };
}

const CRC = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(bytes: Uint8Array): number {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type: string, data: Uint8Array): Uint8Array {
  const out = new Uint8Array(12 + data.length);
  const view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  for (let i = 0; i < 4; i++) out[4 + i] = type.charCodeAt(i);
  out.set(data, 8);
  view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
  return out;
}

/** An RGBA image as a PNG. */
export function png(width: number, height: number, rgba: Uint8Array): Uint8Array {
  const header = new Uint8Array(13);
  const h = new DataView(header.buffer);
  h.setUint32(0, width);
  h.setUint32(4, height);
  header[8] = 8; // bit depth
  header[9] = 6; // RGBA
  const raw = new Uint8Array((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    raw.set(rgba.subarray(y * width * 4, (y + 1) * width * 4), y * (width * 4 + 1) + 1);
  }
  const parts = [
    new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", header),
    chunk("IDAT", new Uint8Array(deflateSync(raw))),
    chunk("IEND", new Uint8Array()),
  ];
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let offset = 0;
  for (const p of parts) {
    out.set(p, offset);
    offset += p.length;
  }
  return out;
}

/** Logs `rgba` as `PNG <name> <base64>`. */
export function logPng(name: string, image: { width: number; height: number; rgba: Uint8Array }): void {
  const bytes = png(image.width, image.height, image.rgba);
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  console.log(`PNG ${name} ${btoa(binary)}`);
}
