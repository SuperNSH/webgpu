// The gallery's probe: what proves a demo drew. Not part of the reference
// app (its e2e harness is a separate WebSocket client); here every demo page
// carries one, and the tests (gallery.test.tsx) read what it reports.
//
// - It counts the GPU errors nobody caught (`uncapturederror`) on the
//   demo's device.
// - It adds COPY_SRC to the canvas's configuration, and copies the canvas
//   texture of one frame (the first, by default) to a buffer before that
//   frame is presented. The copy is summarized as one line of text (size,
//   errors, distinct colors, dominant colors) and logged as a PNG
//   (`PNG <name> <base64>`, scaled down) for the evidence.
// - In snapshot mode (the Home screen's switch), a demo that has a
//   reference snapshot (packages/webgpu/src/__tests__/snapshots) renders
//   that one frame at the snapshot's size, 1024x1024, with `Date.now()`
//   frozen at the time the reference test uses, and reports how many
//   pixels differ from the snapshot (the capture composited over the
//   reference test's clear color, since most demos clear to transparent).

import { context, signal } from "nsh";
import type { RNCanvasContext } from "nsh-webgpu";
import { base64, decodePng, encodePng, type Image, shrink } from "./png";

/** A reference snapshot a demo's frame is compared with. */
export interface Snapshot {
  /** The snapshot's file name (in `snapshots/`). */
  file: string;
  /** Its `asset:/` URL (an `import ... from "...png?url"`). */
  url: string;
  /** What `Date.now()` returns while the frame renders: the reference test's time, in milliseconds. */
  time?: number;
  /** The reference test's clear color (premultiplied RGBA in [0, 1]): the capture is composited over it. */
  clear: [number, number, number, number];
  /** Frames rendered before the one compared; 0 by default. */
  frames?: number;
  /** Milliseconds the frozen time moves at each frame (the reference test's step); 0 by default. */
  step?: number;
}

/** What a demo page tells its probe. */
export interface ProbeOptions {
  /** The frame captured (0: the first). */
  frame?: number;
  /** The reference snapshot of this demo, when the reference has one. */
  snapshot?: Snapshot;
}

/** Whether demos with a reference snapshot render it (the Home screen's switch). */
export const snapshotMode = signal(false);

/** The side of the reference tests' canvas. */
export const SNAPSHOT_SIZE = 1024;

/** A channel difference above this counts a pixel as different (10%, as the reference's checkImage threshold). */
const CHANNEL_TOLERANCE = 26;

export class Probe {
  /** The one-line report the page shows and the tests read. */
  readonly status = signal("");
  /** Uncaught GPU errors, in order. */
  readonly errors = signal<string[]>([]);
  #frames = 0;
  #captured = false;
  #devices = new WeakSet<GPUDevice>();

  constructor(
    readonly name: string,
    readonly options: ProbeOptions = {},
  ) {
    this.status.set(`probe ${name}: waiting`);
  }

  /** Whether this demo renders its reference snapshot now. */
  get snapshot(): Snapshot | undefined {
    return snapshotMode() ? this.options.snapshot : undefined;
  }

  /** Counts the device's uncaught errors. */
  watch(device: GPUDevice): GPUDevice {
    if (this.#devices.has(device)) return device;
    this.#devices.add(device);
    device.addEventListener("uncapturederror", (event) => {
      const message = (event as GPUUncapturedErrorEvent).error.message;
      console.warn(`[gallery] ${this.name}: uncaptured GPU error: ${message}`);
      this.errors.update((list) => [...list, message]);
    });
    return device;
  }

  /**
   * Makes every `configure()` of `ctx` add COPY_SRC (to read the frame
   * back) and, in snapshot mode, size the canvas as the reference test's.
   */
  context<C extends RNCanvasContext>(ctx: C): C {
    const configure = ctx.configure.bind(ctx);
    const probe = this;
    ctx.configure = (config: GPUCanvasConfiguration) => {
      if (probe.snapshot) {
        ctx.canvas.width = SNAPSHOT_SIZE;
        ctx.canvas.height = SNAPSHOT_SIZE;
      }
      probe.watch(config.device);
      configure({ ...config, usage: (config.usage ?? GPUTextureUsage.RENDER_ATTACHMENT) | GPUTextureUsage.COPY_SRC });
    };
    return ctx;
  }

  /** Runs `render` with `Date.now()` frozen at the snapshot's time in snapshot mode. */
  run(render: () => void): void {
    const snapshot = this.snapshot;
    const time = snapshot?.time === undefined ? undefined : snapshot.time + this.#frames * (snapshot.step ?? 0);
    if (time === undefined) {
      render();
      return;
    }
    const now = Date.now;
    Date.now = () => time;
    try {
      render();
    } finally {
      Date.now = now;
    }
  }

  /** Whether the frame just rendered is the one compared with the snapshot (snapshot mode only). */
  get snapshotDone(): boolean {
    return this.#captured && this.snapshot !== undefined;
  }

  /**
   * After a frame's work is submitted and before `present()`: copies the
   * canvas texture when this is the frame to capture. Returns whether it did.
   */
  frame(device: GPUDevice, ctx: RNCanvasContext): boolean {
    const index = this.#frames++;
    const target = this.snapshot ? (this.snapshot.frames ?? 0) : (this.options.frame ?? 0);
    if (this.#captured || index !== target) return false;
    this.#captured = true;
    const texture = ctx.getCurrentTexture();
    const encoder = device.createCommandEncoder({ label: "gallery probe" });
    const read = copyOut(device, encoder, texture);
    device.queue.submit([encoder.finish()]);
    const snapshot = this.snapshot;
    read()
      .then(async (image) => {
        await device.queue.onSubmittedWorkDone();
        await this.report(image, index, snapshot);
      })
      .catch((error: unknown) => this.status.set(`probe ${this.name}: failed: ${String(error)}`));
    return true;
  }

  /** Reports a failure of the demo itself (an exception in its setup or frame). */
  fail(error: unknown): void {
    console.warn(`[gallery] ${this.name}: ${String(error)}`);
    this.status.set(`probe ${this.name}: failed: ${String(error)}`);
  }

  async report(image: Image, frame: number, snapshot?: Snapshot): Promise<void> {
    await turn();
    let line = `probe ${this.name}: ${image.width}x${image.height} frame=${frame} err=${this.errors().length} ${summarize(image)}`;
    logPng(snapshot ? `${this.name}-snapshot` : this.name, image);
    if (snapshot) {
      await turn();
      const reference = decodePng(new Uint8Array(await (await fetch(snapshot.url)).arrayBuffer()));
      const { differ, total } = await compare(image, snapshot.clear, reference);
      line += ` snapshot=${snapshot.file}:${differ}/${total}`;
    }
    this.status.set(line);
  }
}

/** The probe of the demo page around a component (the gallery's DemoPage provides it). */
export const ProbeContext = context<Probe>(new Probe("unnamed"));

/** Copies `texture` into a mappable buffer within `encoder`; resolves to its RGBA rows. */
export function copyOut(device: GPUDevice, encoder: GPUCommandEncoder, texture: GPUTexture): () => Promise<Image> {
  const { width, height } = texture;
  const bytesPerPixel = texture.format === "rgba16float" ? 8 : 4;
  const bytesPerRow = Math.ceil((width * bytesPerPixel) / 256) * 256;
  const buffer = device.createBuffer({
    label: "gallery probe readback",
    size: bytesPerRow * height,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });
  encoder.copyTextureToBuffer({ texture }, { buffer, bytesPerRow }, [width, height]);
  const format = texture.format;
  // An rgba16float canvas's values are shown on an extended-sRGB layer (the
  // sRGB curve, extended past [0, 1]): within [0, 1] they are the PNG's
  // values as they are, clamped.
  return async () => {
    await buffer.mapAsync(GPUMapMode.READ);
    const padded = new Uint8Array(buffer.getMappedRange().slice(0));
    buffer.unmap();
    buffer.destroy();
    const rgba = new Uint8Array(width * height * 4);
    if (format === "rgba16float") {
      const view = new DataView(padded.buffer);
      for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
          for (let c = 0; c < 4; c++) {
            const v = half(view.getUint16(y * bytesPerRow + x * 8 + c * 2, true));
            rgba[(y * width + x) * 4 + c] = Math.round(Math.min(1, Math.max(0, v)) * 255);
          }
        }
      }
      return { width, height, rgba };
    }
    const bgra = format.startsWith("bgra");
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

function half(bits: number): number {
  const sign = bits & 0x8000 ? -1 : 1;
  const exponent = (bits >> 10) & 0x1f;
  const fraction = bits & 0x3ff;
  if (exponent === 0) return sign * 2 ** -14 * (fraction / 1024);
  if (exponent === 0x1f) return fraction ? Number.NaN : sign * Number.POSITIVE_INFINITY;
  return sign * 2 ** (exponent - 15) * (1 + fraction / 1024);
}

/**
 * `distinct=<n> top=<color>:<percent>,...`: colors quantized to 4 bits per
 * channel (`#rgba`), the five most common with their share of the pixels.
 */
export function summarize(image: Image): string {
  const counts = new Map<number, number>();
  const { rgba } = image;
  const total = image.width * image.height;
  for (let i = 0; i < total; i++) {
    const s = i * 4;
    const key = ((rgba[s] >> 4) << 12) | ((rgba[s + 1] >> 4) << 8) | ((rgba[s + 2] >> 4) << 4) | (rgba[s + 3] >> 4);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  const top = [...counts.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
    .map(([key, n]) => `#${key.toString(16).padStart(4, "0")}:${((100 * n) / total).toFixed(1)}`);
  return `distinct=${counts.size} top=${top.join(",")}`;
}

/**
 * Ends this call into JS and goes on in the next one: NSH's tests run an app
 * with a budget per call (2 s), which a whole image's pixels can pass.
 */
const turn = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

/** Pixels compared per call into JS. */
const PIXELS_PER_TURN = 1 << 17;

/**
 * Pixels of `image` (premultiplied, composited over `clear`) whose channels
 * differ from `reference`'s by more than the tolerance; a size mismatch
 * differs everywhere.
 */
async function compare(
  image: Image,
  clear: [number, number, number, number],
  reference: Image,
): Promise<{ differ: number; total: number }> {
  const total = reference.width * reference.height;
  if (image.width !== reference.width || image.height !== reference.height) return { differ: total, total };
  const a = image.rgba;
  const b = reference.rgba;
  let differ = 0;
  for (let start = 0; start < a.length; start += PIXELS_PER_TURN * 4) {
    await turn();
    const end = Math.min(a.length, start + PIXELS_PER_TURN * 4);
    for (let i = start; i < end; i += 4) {
      const rest = (1 - a[i + 3] / 255) * 255;
      for (let c = 0; c < 4; c++) {
        if (Math.abs(Math.round(a[i + c] + rest * clear[c]) - b[i + c]) > CHANNEL_TOLERANCE) {
          differ++;
          break;
        }
      }
    }
  }
  return { differ, total };
}

/** Logs `image` (scaled down to 640 px) as `PNG <name> <base64>`. */
export function logPng(name: string, image: Image): void {
  console.log(`PNG ${name} ${base64(encodePng(shrink(image, 640)))}`);
}
