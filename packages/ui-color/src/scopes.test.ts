import assert from "node:assert/strict";
import test from "node:test";
import { computeParade, computeWaveform, frameFromRgba8 } from "./scopes";

test("waveform puts black at the bottom and white at the top", () => {
  const black = computeWaveform({ width: 2, height: 1, channels: 3, data: [0, 0, 0, 0, 0, 0] }, 4);
  const white = computeWaveform({ width: 2, height: 1, channels: 3, data: [1, 1, 1, 1, 1, 1] }, 4);
  assert.equal(black.values[3 * 2 + 0], 1);
  assert.equal(black.values[0], 0);
  assert.equal(white.values[0], 1);
  assert.equal(white.values[3 * 2], 0);
});

test("waveform follows a gray ramp", () => {
  const grid = computeWaveform(
    {
      width: 3,
      height: 1,
      channels: 3,
      data: [0, 0, 0, 0.5, 0.5, 0.5, 1, 1, 1],
    },
    3,
  );
  assert.equal(grid.values[2 * 3 + 0], 1);
  assert.equal(grid.values[1 * 3 + 1], 1);
  assert.equal(grid.values[0 * 3 + 2], 1);
});

test("parade separates channels", () => {
  const grid = computeParade({ width: 1, height: 1, channels: 3, data: [1, 0, 0] }, 2);
  assert.equal(grid.width, 3);
  assert.equal(grid.values[0 * 3 + 0], 1);
  assert.equal(grid.values[1 * 3 + 1], 1);
  assert.equal(grid.values[1 * 3 + 2], 1);
  assert.equal(grid.values[0 * 3 + 1], 0);
});

test("rejects a short buffer and normalizes 8-bit frames", () => {
  assert.throws(() => computeWaveform({ width: 2, height: 1, channels: 3, data: [0, 0, 0] }));
  const frame = frameFromRgba8(1, 1, [255, 0, 0, 255], 4);
  assert.equal(frame.data[0], 1);
  const parade = computeParade(frame, 2);
  assert.equal(parade.values[0], 1);
});
