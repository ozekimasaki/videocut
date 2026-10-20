import assert from "node:assert/strict";
import test from "node:test";

import { audibleTrackIds, isAudible } from "./audible.ts";

test("mute and solo follow mixer rules", () => {
  assert.equal(isAudible(false, false, false), true);
  assert.equal(isAudible(true, true, true), false);
  assert.equal(isAudible(false, false, true), false);
  assert.equal(isAudible(false, true, true), true);

  const tracks = [
    { id: "a", muted: false, solo: false },
    { id: "b", muted: false, solo: true },
    { id: "c", muted: true, solo: true },
  ];
  assert.deepEqual(audibleTrackIds(tracks), ["b"]);
});
