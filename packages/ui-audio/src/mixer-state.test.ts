import assert from "node:assert/strict";
import test from "node:test";

import type { Sequence } from "@videocut/model";

import { stripViews } from "./mixer-state.ts";

test("strip views join mute from the sequence and solo from the mixer", () => {
  const sequence = {
    audioTracks: [
      { id: "a", index: 0, muted: false },
      { id: "b", index: 1, muted: true },
    ],
  } as Sequence;
  const views = stripViews(
    sequence,
    {
      masterGainDb: 0,
      strips: [
        { trackId: "a", solo: false, faderDb: -3, pan: -1 },
        { trackId: "b", solo: true, faderDb: 0, pan: 0 },
      ],
    },
    { a: [{ peakDbfs: -12 }] },
  );
  assert.equal(views[0]?.name, "A1");
  assert.equal(views[0]?.audible, false);
  assert.equal(views[0]?.faderDb, -3);
  assert.equal(views[1]?.audible, false);
  assert.equal(views[1]?.muted, true);
  assert.deepEqual(views[0]?.meters, [{ peakDbfs: -12 }]);
});
