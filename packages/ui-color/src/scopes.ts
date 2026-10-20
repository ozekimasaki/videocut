/** RGB or RGBA frame. Samples are 0..1, row-major. */
export interface RgbFrame {
  width: number;
  height: number;
  data: ArrayLike<number>;
  channels: 3 | 4;
}

/** Density image. Row 0 is white, the last row is black. */
export interface ScopeGrid {
  width: number;
  height: number;
  values: Float32Array;
}

export type ScopeMode = "waveform" | "parade";

const LUMA_R = 0.2126;
const LUMA_G = 0.7152;
const LUMA_B = 0.0722;

export function computeWaveform(frame: RgbFrame, levels = 256): ScopeGrid {
  assertFrame(frame, levels);
  const values = new Float32Array(frame.width * levels);
  let max = 0;
  for (let y = 0; y < frame.height; y += 1) {
    for (let x = 0; x < frame.width; x += 1) {
      const sample = readPixel(frame, x, y);
      const luma = LUMA_R * sample.r + LUMA_G * sample.g + LUMA_B * sample.b;
      const row = levelRow(luma, levels);
      const index = row * frame.width + x;
      values[index] += 1;
      max = Math.max(max, values[index]);
    }
  }
  return normalize(values, frame.width, levels, max);
}

export function computeParade(frame: RgbFrame, levels = 256): ScopeGrid {
  assertFrame(frame, levels);
  const width = frame.width * 3;
  const values = new Float32Array(width * levels);
  let max = 0;
  for (let y = 0; y < frame.height; y += 1) {
    for (let x = 0; x < frame.width; x += 1) {
      const sample = readPixel(frame, x, y);
      const channels = [sample.r, sample.g, sample.b];
      for (let channel = 0; channel < channels.length; channel += 1) {
        const column = channel * frame.width + x;
        const row = levelRow(channels[channel], levels);
        const index = row * width + column;
        values[index] += 1;
        max = Math.max(max, values[index]);
      }
    }
  }
  return normalize(values, width, levels, max);
}

export function frameFromRgba8(
  width: number,
  height: number,
  data: ArrayLike<number>,
  channels: 3 | 4,
): RgbFrame {
  const normalized = new Float32Array(data.length);
  for (let index = 0; index < data.length; index += 1) {
    normalized[index] = data[index] / 255;
  }
  return { width, height, data: normalized, channels };
}

function levelRow(value: number, levels: number): number {
  const clamped = Math.min(1, Math.max(0, value));
  return Math.round((1 - clamped) * (levels - 1));
}

function readPixel(frame: RgbFrame, x: number, y: number): { r: number; g: number; b: number } {
  const index = (y * frame.width + x) * frame.channels;
  return {
    r: frame.data[index],
    g: frame.data[index + 1],
    b: frame.data[index + 2],
  };
}

function normalize(values: Float32Array, width: number, height: number, max: number): ScopeGrid {
  if (max > 0) {
    for (let index = 0; index < values.length; index += 1) {
      values[index] /= max;
    }
  }
  return { width, height, values };
}

function assertFrame(frame: RgbFrame, levels: number): void {
  if (!Number.isInteger(frame.width) || frame.width <= 0) {
    throw new Error("frame width must be a positive integer");
  }
  if (!Number.isInteger(frame.height) || frame.height <= 0) {
    throw new Error("frame height must be a positive integer");
  }
  if (!Number.isInteger(levels) || levels < 2) {
    throw new Error("scope levels must be an integer of at least 2");
  }
  if (frame.channels !== 3 && frame.channels !== 4) {
    throw new Error("frame channels must be 3 or 4");
  }
  const needed = frame.width * frame.height * frame.channels;
  if (frame.data.length < needed) {
    throw new Error(`frame data needs ${needed} samples`);
  }
}
