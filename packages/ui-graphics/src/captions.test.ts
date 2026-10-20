import assert from "node:assert/strict";
import test from "node:test";
import type { Sequence } from "@videocut/model";
import {
  commitCaptionTiming,
  updateCaptionText,
  updateCaptionTiming,
  withCaptionTiming,
} from "./captions.ts";
import { msToTicks } from "./timing.ts";

const sequence: Sequence = {
  id: "seq-1",
  name: "seq",
  settings: {
    width: 1920,
    height: 1080,
    fps: { num: 30, den: 1 },
    sampleRate: 48_000,
    audioChannels: 2,
    startTimecodeTicks: 0,
  },
  videoTracks: [],
  audioTracks: [],
  captions: [{ id: "cap-1", startTicks: msToTicks(1_000), endTicks: msToTicks(2_000), text: "Hello" }],
};

test("caption timing rejects an inverted range and keeps the sequence", () => {
  const caption = sequence.captions?.[0];
  assert.ok(caption);
  assert.equal(withCaptionTiming(caption, 20, 20).ok, false);
  const updated = updateCaptionTiming(sequence, "cap-1", 20, 10);
  assert.equal(updated.ok, false);
  assert.equal(sequence.captions?.[0]?.startTicks, msToTicks(1_000));
});

test("caption timing and text updates are copied onto the sequence", () => {
  const startTicks = msToTicks(1_500);
  const endTicks = msToTicks(3_000);
  const timed = updateCaptionTiming(sequence, "cap-1", startTicks, endTicks);
  assert.equal(timed.ok, true);
  if (!timed.ok) {
    return;
  }
  assert.equal(timed.value.captions?.[0]?.startTicks, startTicks);
  assert.equal(timed.value.captions?.[0]?.endTicks, endTicks);
  assert.equal(sequence.captions?.[0]?.text, "Hello");
  const text = updateCaptionText(timed.value, "cap-1", "Updated");
  assert.equal(text.ok, true);
  if (!text.ok) {
    return;
  }
  assert.equal(text.value.captions?.[0]?.text, "Updated");
  assert.equal(updateCaptionText(sequence, "missing", "x").ok, false);
});

test("timecode commit parses a pair and reports bad input", () => {
  const committed = commitCaptionTiming("00:00:01.000", "00:00:04.500");
  assert.equal(committed.ok, true);
  if (!committed.ok) {
    return;
  }
  assert.equal(committed.value.startTicks, msToTicks(1_000));
  assert.equal(committed.value.endTicks, msToTicks(4_500));
  assert.equal(commitCaptionTiming("nope", "00:00:01.000").ok, false);
  assert.equal(commitCaptionTiming("00:00:04.000", "00:00:01.000").ok, false);
});
