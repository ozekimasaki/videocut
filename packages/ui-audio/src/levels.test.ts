import assert from "node:assert/strict";
import test from "node:test";

import { clampFader, clampPan, formatDb, formatPan, meterFraction } from "./levels.ts";

test("meter and fader formatting", () => {
  assert.equal(meterFraction(0), 1);
  assert.equal(meterFraction(-60), 0);
  assert.equal(meterFraction(-30), 0.5);
  assert.equal(meterFraction(3), 1);
  assert.equal(meterFraction(Number.NEGATIVE_INFINITY), 0);
  assert.equal(formatDb(3.04), "+3.0 dB");
  assert.equal(formatDb(-6.04), "-6.0 dB");
  assert.equal(formatDb(Number.NaN), "-inf dB");
  assert.equal(formatPan(0), "C");
  assert.equal(formatPan(-1), "L100");
  assert.equal(formatPan(0.5), "R50");
  assert.equal(clampFader(80), 12);
  assert.equal(clampPan(-4), -1);
});
