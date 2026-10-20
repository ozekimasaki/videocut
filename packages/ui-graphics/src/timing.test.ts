import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { formatTicks, msToTicks, parseTimecode, TICKS_PER_SECOND, ticksToMs } from "./timing.ts";

test("one and a half seconds round-trips through milliseconds", () => {
  const ticks = (TICKS_PER_SECOND * 3) / 2;
  assert.equal(msToTicks(1_500), ticks);
  assert.equal(ticksToMs(ticks), 1_500);
  assert.equal(formatTicks(ticks), "00:00:01.500");
  assert.equal(parseTimecode("00:00:01.500"), ticks);
  assert.equal(parseTimecode("00:00:01,500"), ticks);
});

test("half a millisecond rounds away from zero", () => {
  const half = TICKS_PER_SECOND / 1000 / 2;
  assert.equal(ticksToMs(half), 1);
  assert.equal(ticksToMs(half - 1), 0);
});

test("tick rate matches the shared model", () => {
  const source = readFileSync(new URL("../../model/src/types.ts", import.meta.url), "utf8");
  assert.match(source, /export const TICKS_PER_SECOND = 254_016_000_000;/);
});

test("timecode parsing rejects incomplete and out-of-range values", () => {
  assert.equal(parseTimecode(""), null);
  assert.equal(parseTimecode("00:60:00.000"), null);
  assert.equal(parseTimecode("1:02:03.004"), msToTicks(3_723_004));
  assert.equal(formatTicks(parseTimecode("1:02:03.004") ?? 0), "01:02:03.004");
  assert.equal(formatTicks(0), "00:00:00.000");
});
