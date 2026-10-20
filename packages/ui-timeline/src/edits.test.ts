import assert from "node:assert/strict";
import test from "node:test";
import { TICKS_PER_SECOND, type Clip, type Sequence, type Track } from "@videocut/model";
import {
  EditError,
  addMarker,
  addMulticamAngle,
  extract,
  freezeFrame,
  insert,
  lift,
  linkClips,
  moveMarker,
  nest,
  overwrite,
  razor,
  removeMarker,
  rippleDelete,
  rippleTrim,
  rollTrim,
  setDuration,
  setReversed,
  setSpeed,
  setSyncLock,
  setTrackLocked,
  setTrackTargeted,
  slide,
  slip,
  snapTicks,
  sourceAt,
  switchMulticamAngle,
  unlinkClip,
  collectSnapPoints,
} from "./edits.ts";

function ticks(seconds: number): number {
  return seconds * TICKS_PER_SECOND;
}

function span(name: string, startSeconds: number, endSeconds: number): Clip {
  return {
    id: crypto.randomUUID(),
    name,
    mediaId: "media-1",
    kind: "media",
    timelineIn: ticks(startSeconds),
    timelineOut: ticks(endSeconds),
    sourceIn: 0,
    sourceOut: ticks(endSeconds - startSeconds),
    speed: 1,
    reversed: false,
    enabled: true,
    labelColor: 0,
  };
}

function track(index: number, kind: Track["kind"], clips: Clip[]): Track {
  return {
    id: crypto.randomUUID(),
    index,
    kind,
    clips,
    locked: false,
    muted: false,
    hidden: false,
    syncLock: true,
    targeted: true,
  };
}

function sequence(video: Clip[], audio: Clip[]): Sequence {
  return {
    id: "seq",
    name: "seq",
    settings: {
      width: 1920,
      height: 1080,
      fps: { num: 30, den: 1 },
      sampleRate: 48_000,
      audioChannels: 2,
      startTimecodeTicks: 0,
    },
    videoTracks: [track(0, "video", video)],
    audioTracks: [track(0, "audio", audio)],
    markers: [],
    multicamAngles: [],
  };
}

function find(seq: Sequence, name: string, atSeconds: number): Clip {
  const at = ticks(atSeconds);
  const clip = [...seq.videoTracks, ...seq.audioTracks]
    .flatMap((item) => item.clips)
    .find((item) => item.name === name && item.timelineIn <= at && item.timelineOut > at);
  assert.ok(clip, `missing ${name} at ${atSeconds}s`);
  return clip;
}

function namesAt(seq: Sequence, video: boolean): Array<[string, number, number]> {
  const chosen = video ? seq.videoTracks : seq.audioTracks;
  return chosen[0]!.clips.map((clip) => [clip.name, clip.timelineIn, clip.timelineOut]);
}

function code(run: () => void): string {
  try {
    run();
  } catch (error) {
    assert.ok(error instanceof EditError);
    return error.code;
  }
  assert.fail("expected an edit error");
}

test("insert shifts later clips and sync-locked tracks", () => {
  const seq = sequence([span("V", 0, 2)], [span("A", 0, 2)]);
  insert(seq, ticks(1), [{ trackId: seq.videoTracks[0]!.id, clip: span("NEW", 0, 1) }]);
  assert.deepEqual(namesAt(seq, true), [
    ["V", ticks(0), ticks(1)],
    ["NEW", ticks(1), ticks(2)],
    ["V", ticks(2), ticks(3)],
  ]);
  assert.deepEqual(namesAt(seq, false), [
    ["A", ticks(0), ticks(1)],
    ["A", ticks(2), ticks(3)],
  ]);
  assert.equal(find(seq, "V", 0).sourceOut, ticks(1));
  assert.equal(find(seq, "V", 2).sourceIn, ticks(1));
});

test("insert rejects an untargeted track", () => {
  const seq = sequence([], []);
  const videoId = seq.videoTracks[0]!.id;
  setTrackTargeted(seq, videoId, false);
  assert.equal(code(() => insert(seq, 0, [{ trackId: videoId, clip: span("NEW", 0, 1) }])), "track-not-targeted");
  assert.equal(seq.videoTracks[0]!.clips.length, 0);
});

test("overwrite replaces a range without shifting", () => {
  const seq = sequence([span("V", 0, 4), span("TAIL", 4, 6)], [span("A", 0, 4)]);
  overwrite(seq, seq.videoTracks[0]!.id, span("NEW", 0, 2), ticks(1));
  assert.deepEqual(namesAt(seq, true), [
    ["V", ticks(0), ticks(1)],
    ["NEW", ticks(1), ticks(3)],
    ["V", ticks(3), ticks(4)],
    ["TAIL", ticks(4), ticks(6)],
  ]);
  assert.equal(find(seq, "V", 0).sourceOut, ticks(1));
  assert.equal(find(seq, "V", 3).sourceIn, ticks(3));
  assert.deepEqual(namesAt(seq, false), [["A", ticks(0), ticks(4)]]);
});

test("lift leaves a gap on targeted tracks only", () => {
  const seq = sequence([span("V", 0, 4)], [span("A", 0, 4)]);
  setTrackTargeted(seq, seq.audioTracks[0]!.id, false);
  const removed = lift(seq, ticks(1), ticks(3));
  assert.equal(removed.length, 1);
  assert.deepEqual(namesAt(seq, true), [
    ["V", ticks(0), ticks(1)],
    ["V", ticks(3), ticks(4)],
  ]);
  assert.deepEqual(namesAt(seq, false), [["A", ticks(0), ticks(4)]]);
});

test("extract closes the gap and ripples sync-locked tracks", () => {
  const seq = sequence([span("V", 0, 4), span("TAIL", 4, 6)], [span("A", 0, 6)]);
  setTrackTargeted(seq, seq.audioTracks[0]!.id, false);
  extract(seq, ticks(1), ticks(3));
  assert.deepEqual(namesAt(seq, true), [
    ["V", ticks(0), ticks(1)],
    ["V", ticks(1), ticks(2)],
    ["TAIL", ticks(2), ticks(4)],
  ]);
  assert.deepEqual(namesAt(seq, false), [
    ["A", ticks(0), ticks(1)],
    ["A", ticks(1), ticks(4)],
  ]);
  assert.equal(find(seq, "A", 1).sourceIn, ticks(3));
});

test("sync lock off keeps the other track still", () => {
  const seq = sequence([span("V", 0, 4)], [span("A", 0, 4)]);
  setSyncLock(seq, seq.audioTracks[0]!.id, false);
  setTrackTargeted(seq, seq.audioTracks[0]!.id, false);
  extract(seq, ticks(1), ticks(2));
  assert.deepEqual(namesAt(seq, false), [["A", ticks(0), ticks(4)]]);
  assert.equal(find(seq, "V", 1).timelineIn, ticks(1));
});

test("razor splits linked audio and video", () => {
  const video = span("V", 0, 4);
  const audio = span("A", 0, 4);
  video.linkedIds = [audio.id];
  audio.linkedIds = [video.id];
  const seq = sequence([video], [audio]);
  const pairs = razor(seq, ticks(2));
  assert.equal(pairs.length, 2);
  assert.ok(pairs.some(([left]) => left === video.id));
  const left = find(seq, "V", 0);
  const right = find(seq, "V", 2);
  assert.deepEqual([left.timelineIn, left.timelineOut], [ticks(0), ticks(2)]);
  assert.deepEqual([left.sourceIn, left.sourceOut], [0, ticks(2)]);
  assert.deepEqual([right.timelineIn, right.timelineOut], [ticks(2), ticks(4)]);
  assert.deepEqual([right.sourceIn, right.sourceOut], [ticks(2), ticks(4)]);
  assert.equal(left.linkedIds?.length, 1);
  assert.equal(right.linkedIds?.length, 1);
  assert.notDeepEqual(left.linkedIds, right.linkedIds);
  assert.deepEqual(find(seq, "A", 2).linkedIds, [right.id]);
});

test("razor maps reversed and double-speed source", () => {
  const reversed = span("REV", 0, 4);
  reversed.reversed = true;
  const fast = span("FAST", 0, 2);
  fast.sourceOut = ticks(4);
  fast.speed = 2;
  const seq = sequence([reversed], []);
  seq.videoTracks.push(track(1, "video", [fast]));
  razor(seq, ticks(1));
  const left = find(seq, "REV", 0);
  const right = find(seq, "REV", 1);
  assert.deepEqual([left.sourceIn, left.sourceOut], [ticks(3), ticks(4)]);
  assert.deepEqual([right.sourceIn, right.sourceOut], [0, ticks(3)]);
  assert.equal(sourceAt(left, ticks(0)), ticks(4));
  assert.equal(find(seq, "FAST", 0).sourceOut, ticks(2));
  assert.equal(find(seq, "FAST", 1).sourceIn, ticks(2));
});

test("ripple trim out moves followers", () => {
  const seq = sequence([span("A", 0, 2), span("B", 2, 4)], [span("S", 0, 4)]);
  setTrackTargeted(seq, seq.audioTracks[0]!.id, false);
  rippleTrim(seq, find(seq, "A", 0).id, "out", -ticks(1));
  assert.equal(find(seq, "A", 0).timelineOut, ticks(1));
  assert.equal(find(seq, "A", 0).sourceOut, ticks(1));
  assert.equal(find(seq, "B", 1).timelineIn, ticks(1));
  assert.equal(find(seq, "B", 1).timelineOut, ticks(3));
  assert.deepEqual(namesAt(seq, false), [
    ["S", ticks(0), ticks(1)],
    ["S", ticks(1), ticks(3)],
  ]);
  assert.equal(find(seq, "S", 1).sourceIn, ticks(2));
});

test("ripple trim in changes the source head and keeps the cut", () => {
  const clip = span("A", 0, 4);
  clip.sourceIn = ticks(2);
  clip.sourceOut = ticks(6);
  const seq = sequence([clip, span("B", 4, 6)], []);
  rippleTrim(seq, find(seq, "A", 0).id, "in", ticks(1));
  const trimmed = find(seq, "A", 0);
  assert.equal(trimmed.timelineIn, ticks(0));
  assert.equal(trimmed.timelineOut, ticks(5));
  assert.equal(trimmed.sourceIn, ticks(1));
  assert.equal(find(seq, "B", 5).timelineIn, ticks(5));
});

test("ripple trim rejects missing media without changing the sequence", () => {
  const seq = sequence([span("A", 0, 2)], []);
  const before = structuredClone(seq);
  assert.equal(code(() => rippleTrim(seq, find(seq, "A", 0).id, "in", ticks(1))), "insufficient-source");
  assert.deepEqual(seq, before);
});

test("roll trim moves the cut between adjacent clips", () => {
  const right = span("B", 2, 4);
  right.sourceIn = ticks(1);
  right.sourceOut = ticks(3);
  const seq = sequence([span("A", 0, 2), right], []);
  rollTrim(seq, find(seq, "A", 0).id, ticks(1));
  const left = find(seq, "A", 0);
  const moved = find(seq, "B", 3);
  assert.deepEqual([left.timelineOut, left.sourceOut], [ticks(3), ticks(3)]);
  assert.deepEqual([moved.timelineIn, moved.timelineOut], [ticks(3), ticks(4)]);
  assert.deepEqual([moved.sourceIn, moved.sourceOut], [ticks(2), ticks(3)]);
});

test("roll trim requires adjacent clips", () => {
  const seq = sequence([span("A", 0, 2), span("B", 3, 4)], []);
  assert.equal(code(() => rollTrim(seq, find(seq, "A", 0).id, ticks(1))), "not-adjacent");
});

test("slip moves the source window", () => {
  const clip = span("A", 0, 2);
  clip.sourceIn = ticks(1);
  clip.sourceOut = ticks(3);
  const seq = sequence([clip], []);
  const clipId = find(seq, "A", 0).id;
  slip(seq, clipId, -ticks(1));
  const slipped = find(seq, "A", 0);
  assert.deepEqual([slipped.timelineIn, slipped.timelineOut], [ticks(0), ticks(2)]);
  assert.deepEqual([slipped.sourceIn, slipped.sourceOut], [0, ticks(2)]);
  assert.equal(code(() => slip(seq, clipId, -ticks(1))), "insufficient-source");
});

test("slide moves a clip between its neighbors", () => {
  const tail = span("C", 4, 6);
  tail.sourceIn = ticks(1);
  tail.sourceOut = ticks(3);
  const seq = sequence([span("A", 0, 2), span("B", 2, 4), tail], []);
  slide(seq, find(seq, "B", 2).id, ticks(1));
  assert.equal(find(seq, "A", 0).timelineOut, ticks(3));
  assert.equal(find(seq, "A", 0).sourceOut, ticks(3));
  assert.deepEqual([find(seq, "B", 3).timelineIn, find(seq, "B", 3).timelineOut], [ticks(3), ticks(5)]);
  assert.equal(find(seq, "B", 3).sourceIn, 0);
  assert.equal(find(seq, "C", 5).timelineIn, ticks(5));
  assert.equal(find(seq, "C", 5).sourceIn, ticks(2));
  assert.equal(find(seq, "C", 5).timelineOut, ticks(6));
});

test("ripple delete removes a linked pair and closes the gap once", () => {
  const video = span("B", 2, 4);
  const audio = span("B", 2, 4);
  video.linkedIds = [audio.id];
  audio.linkedIds = [video.id];
  const seq = sequence(
    [span("A", 0, 2), video, span("C", 4, 6)],
    [span("A", 0, 2), audio, span("C", 4, 6)],
  );
  rippleDelete(seq, [video.id]);
  assert.deepEqual(namesAt(seq, true), [
    ["A", ticks(0), ticks(2)],
    ["C", ticks(2), ticks(4)],
  ]);
  assert.deepEqual(namesAt(seq, false), [
    ["A", ticks(0), ticks(2)],
    ["C", ticks(2), ticks(4)],
  ]);
});

test("track targeting and lock guard edits", () => {
  const seq = sequence([span("V", 0, 2)], []);
  setTrackLocked(seq, seq.videoTracks[0]!.id, true);
  assert.equal(code(() => lift(seq, 0, ticks(1))), "track-locked");
  assert.equal(seq.videoTracks[0]!.clips.length, 1);
});

test("snapping prefers the nearest clip edge, marker, or playhead", () => {
  const seq = sequence([span("V", 0, 2)], []);
  const markerId = addMarker(seq, {
    timeTicks: ticks(5),
    durationTicks: 0,
    name: "note",
    comment: "look here",
    color: 2,
    kind: "comment",
  });
  moveMarker(seq, markerId, ticks(4));
  assert.equal(snapTicks(seq, ticks(4) - 10, ticks(1), ticks(3)), ticks(4));
  const points = collectSnapPoints(seq, ticks(3));
  assert.ok(points.some(([time, kind]) => time === ticks(2) && kind === "clip-out"));
  assert.ok(points.some(([time, kind]) => time === ticks(3) && kind === "playhead"));
  assert.ok(points.some(([time, kind]) => time === ticks(4) && kind === "marker"));
  assert.equal(removeMarker(seq, markerId).name, "note");
  assert.equal(seq.markers?.length, 0);
});

test("link and unlink keep both sides in agreement", () => {
  const seq = sequence([span("V", 0, 2)], [span("A", 0, 2)]);
  const videoId = find(seq, "V", 0).id;
  const audioId = find(seq, "A", 0).id;
  linkClips(seq, [videoId, audioId]);
  assert.deepEqual(find(seq, "V", 0).linkedIds, [audioId]);
  assert.deepEqual(find(seq, "A", 0).linkedIds, [videoId]);
  unlinkClip(seq, videoId);
  assert.deepEqual(find(seq, "V", 0).linkedIds, []);
  assert.deepEqual(find(seq, "A", 0).linkedIds, []);
});

test("set speed changes duration and set duration changes speed", () => {
  const seq = sequence([span("V", 0, 2), span("TAIL", 2, 3)], [span("A", 0, 2)]);
  const clipId = find(seq, "V", 0).id;
  setSpeed(seq, clipId, 2, true);
  const fast = find(seq, "V", 0);
  assert.equal(fast.speed, 2);
  assert.deepEqual([fast.sourceIn, fast.sourceOut], [0, ticks(2)]);
  assert.equal(fast.timelineOut, ticks(1));
  assert.equal(find(seq, "TAIL", 1).timelineIn, ticks(1));
  assert.equal(find(seq, "A", 0).timelineOut, ticks(1));
  assert.equal(code(() => setDuration(seq, clipId, ticks(4), false)), "overlap");
  setDuration(seq, clipId, ticks(4), true);
  const longer = find(seq, "V", 0);
  assert.equal(longer.timelineOut, ticks(4));
  assert.ok(Math.abs(longer.speed - 0.5) < 1e-9);
  assert.equal(longer.sourceOut, ticks(2));
  assert.equal(find(seq, "TAIL", 4).timelineIn, ticks(4));
});

test("reverse flips playback without moving the clip", () => {
  const seq = sequence([span("V", 0, 2)], []);
  const clipId = find(seq, "V", 0).id;
  setReversed(seq, clipId, true);
  const clip = find(seq, "V", 0);
  assert.equal(clip.reversed, true);
  assert.deepEqual([clip.timelineIn, clip.timelineOut], [ticks(0), ticks(2)]);
  assert.equal(sourceAt(clip, ticks(0)), ticks(2));
  assert.equal(sourceAt(clip, ticks(2)), 0);
});

test("nest replaces clips with one nested sequence", () => {
  const seq = sequence([span("A", 0, 2), span("B", 2, 5)], []);
  const child = nest(seq, [find(seq, "A", 0).id, find(seq, "B", 2).id], "Nest");
  assert.equal(child.videoTracks.length, 1);
  assert.equal(child.videoTracks[0]!.clips.length, 2);
  assert.equal(child.videoTracks[0]!.clips[0]!.timelineIn, 0);
  assert.equal(child.videoTracks[0]!.clips[1]!.timelineIn, ticks(2));
  assert.equal(seq.videoTracks[0]!.clips.length, 1);
  const nested = seq.videoTracks[0]!.clips[0]!;
  assert.equal(nested.kind, "nested");
  assert.equal(nested.nestedSequenceId, child.id);
  assert.deepEqual([nested.timelineIn, nested.timelineOut], [0, ticks(5)]);
  assert.equal(nested.mediaId, undefined);
});

test("switch multicam angle splits and realigns source", () => {
  const seq = sequence([span("CAM", 0, 4)], []);
  const clipId = find(seq, "CAM", 0).id;
  addMulticamAngle(seq, "A", "media-1", 0);
  const angleB = addMulticamAngle(seq, "B", "media-b", ticks(10));
  const switched = switchMulticamAngle(seq, clipId, ticks(2), angleB);
  const left = find(seq, "CAM", 0);
  const right = find(seq, "CAM", 2);
  assert.equal(left.mediaId, "media-1");
  assert.deepEqual([left.sourceIn, left.sourceOut], [0, ticks(2)]);
  assert.equal(right.id, switched);
  assert.equal(right.mediaId, "media-b");
  assert.deepEqual([right.sourceIn, right.sourceOut], [ticks(12), ticks(14)]);
});

test("freeze frame inserts a hold and pushes the tail", () => {
  const seq = sequence([span("V", 0, 4), span("TAIL", 4, 6)], []);
  const freezeId = freezeFrame(seq, find(seq, "V", 0).id, ticks(2), ticks(1));
  const freeze = seq.videoTracks[0]!.clips.find((clip) => clip.id === freezeId);
  assert.ok(freeze);
  assert.equal(freeze.speed, 0);
  assert.deepEqual([freeze.sourceIn, freeze.sourceOut], [ticks(2), ticks(2)]);
  assert.deepEqual([freeze.timelineIn, freeze.timelineOut], [ticks(2), ticks(3)]);
  assert.equal(find(seq, "V", 0).timelineOut, ticks(2));
  assert.equal(find(seq, "V", 3).timelineIn, ticks(3));
  assert.equal(find(seq, "V", 3).sourceIn, ticks(2));
  assert.equal(find(seq, "TAIL", 5).timelineIn, ticks(5));
});

test("locked sync track rejects an insert and keeps the sequence", () => {
  const seq = sequence([span("V", 0, 2)], [span("A", 0, 2)]);
  const before = structuredClone(seq);
  setTrackLocked(seq, seq.audioTracks[0]!.id, true);
  assert.equal(
    code(() => insert(seq, ticks(1), [{ trackId: seq.videoTracks[0]!.id, clip: span("NEW", 0, 1) }])),
    "track-locked",
  );
  seq.audioTracks[0]!.locked = false;
  assert.deepEqual(seq.videoTracks[0]!.clips, before.videoTracks[0]!.clips);
  assert.deepEqual(seq.audioTracks[0]!.clips, before.audioTracks[0]!.clips);
});
