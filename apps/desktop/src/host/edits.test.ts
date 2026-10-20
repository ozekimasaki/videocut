import { TICKS_PER_SECOND, type Project, type Sequence, type Track } from "@videocut/model";
import { describe, expect, it } from "vitest";
import { seconds } from "../model/timecode";
import {
  moveClips,
  overwrite,
  razorAtPlayhead,
  rippleDelete,
  rippleTrimNext,
  rippleTrimPrevious,
  rollEdit,
  slideClip,
  slipClip,
  trimEdge,
} from "./edits";

function clip(id: string, start: number, end: number, extra: Partial<Track["clips"][number]> = {}) {
  return {
    id,
    name: id,
    kind: "media" as const,
    timelineIn: seconds(start),
    timelineOut: seconds(end),
    sourceIn: seconds(start),
    sourceOut: seconds(end),
    speed: 1,
    reversed: false,
    enabled: true,
    labelColor: 0,
    ...extra,
  };
}

function track(id: string, kind: Track["kind"], clips: Track["clips"]): Track {
  return {
    id,
    index: 0,
    kind,
    clips,
    locked: false,
    muted: false,
    hidden: false,
    syncLock: true,
    targeted: true,
  };
}

function sequence(video: Track["clips"], audio: Track["clips"] = []): Sequence {
  return {
    id: "seq",
    name: "seq",
    settings: {
      width: 1920,
      height: 1080,
      fps: { num: 30, den: 1 },
      sampleRate: 48000,
      audioChannels: 2,
      startTimecodeTicks: 0,
    },
    videoTracks: [track("v1", "video", video)],
    audioTracks: [track("a1", "audio", audio)],
  };
}

function wrap(seq: Sequence): Project {
  return { id: "proj", name: "demo", bins: [], sequences: [seq], media: [] };
}

describe("razorAtPlayhead", () => {
  it("splits a clip and its linked audio", () => {
    const project = wrap(sequence([clip("v", 0, 10, { linkedIds: ["a"], sourceIn: 0, sourceOut: seconds(10) })], [clip("a", 0, 10, { linkedIds: ["v"], sourceIn: 0, sourceOut: seconds(10) })]));
    const next = razorAtPlayhead(project, "seq", seconds(4));
    const video = next.sequences[0]!.videoTracks[0]!.clips;
    const audio = next.sequences[0]!.audioTracks[0]!.clips;
    expect(video).toHaveLength(2);
    expect(audio).toHaveLength(2);
    expect(video[0]!.timelineOut).toBe(seconds(4));
    expect(video[1]!.timelineIn).toBe(seconds(4));
    expect(video[0]!.sourceOut).toBe(seconds(4));
    expect(video[1]!.sourceIn).toBe(seconds(4));
    expect(video[0]!.linkedIds).toEqual(["a"]);
    expect(video[1]!.linkedIds?.[0]).toBe(audio[1]!.id);
  });

  it("honors speed when splitting source time", () => {
    const project = wrap(sequence([clip("v", 0, 10, { speed: 2, sourceIn: 0, sourceOut: seconds(20) })]));
    const next = razorAtPlayhead(project, "seq", seconds(4));
    const [left, right] = next.sequences[0]!.videoTracks[0]!.clips;
    expect(left!.sourceOut).toBe(seconds(8));
    expect(right!.sourceIn).toBe(seconds(8));
  });

  it("leaves the project alone when the playhead is on a cut", () => {
    const project = wrap(sequence([clip("v", 0, 10)]));
    expect(razorAtPlayhead(project, "seq", seconds(10))).toBe(project);
  });
});

describe("ripple trim", () => {
  it("Q removes the head and pulls later clips left", () => {
    const project = wrap(sequence([clip("a", 0, 10), clip("b", 10, 15)]));
    const next = rippleTrimPrevious(project, "seq", seconds(4));
    const clips = next.sequences[0]!.videoTracks[0]!.clips;
    expect(clips[0]).toMatchObject({ timelineIn: 0, timelineOut: seconds(6), sourceIn: seconds(4), sourceOut: seconds(10) });
    expect(clips[1]).toMatchObject({ timelineIn: seconds(6), timelineOut: seconds(11) });
  });

  it("W removes the tail and pulls later clips left", () => {
    const project = wrap(sequence([clip("a", 0, 10), clip("b", 10, 15)]));
    const next = rippleTrimNext(project, "seq", seconds(4));
    const clips = next.sequences[0]!.videoTracks[0]!.clips;
    expect(clips[0]).toMatchObject({ timelineIn: 0, timelineOut: seconds(4), sourceOut: seconds(4) });
    expect(clips[1]).toMatchObject({ timelineIn: seconds(4), timelineOut: seconds(9) });
  });
});

describe("trim, roll, slip, slide, move, delete, overwrite", () => {
  it("trims an edge without moving the neighbor", () => {
    const project = wrap(sequence([clip("a", 0, 10), clip("b", 10, 16)]));
    const next = trimEdge(project, "seq", "a", "end", seconds(8));
    const clips = next.sequences[0]!.videoTracks[0]!.clips;
    expect(clips[0]!.timelineOut).toBe(seconds(8));
    expect(clips[1]!.timelineIn).toBe(seconds(10));
  });

  it("rolls the cut between adjacent clips", () => {
    const project = wrap(sequence([clip("a", 0, 10), clip("b", 10, 20)]));
    const next = rollEdit(project, "seq", "a", "end", seconds(12));
    const clips = next.sequences[0]!.videoTracks[0]!.clips;
    expect(clips[0]).toMatchObject({ timelineOut: seconds(12), sourceOut: seconds(12) });
    expect(clips[1]).toMatchObject({ timelineIn: seconds(12), sourceIn: seconds(12), timelineOut: seconds(20) });
  });

  it("slips source without moving the timeline span", () => {
    const project = wrap(sequence([clip("a", 0, 4, { sourceIn: seconds(2), sourceOut: seconds(6) })]));
    const next = slipClip(project, "seq", "a", seconds(1));
    expect(next.sequences[0]!.videoTracks[0]!.clips[0]).toMatchObject({
      timelineIn: 0,
      timelineOut: seconds(4),
      sourceIn: seconds(3),
      sourceOut: seconds(7),
    });
  });

  it("slides a clip between its neighbors", () => {
    const project = wrap(sequence([clip("a", 0, 10), clip("b", 10, 20), clip("c", 20, 30)]));
    const next = slideClip(project, "seq", "b", seconds(2));
    const clips = next.sequences[0]!.videoTracks[0]!.clips;
    expect(clips[0]!.timelineOut).toBe(seconds(12));
    expect(clips[1]).toMatchObject({ timelineIn: seconds(12), timelineOut: seconds(22), sourceIn: seconds(10) });
    expect(clips[2]).toMatchObject({ timelineIn: seconds(22), sourceIn: seconds(22) });
  });

  it("moves a clip until it touches the next one", () => {
    const project = wrap(sequence([clip("a", 0, 5), clip("b", 10, 12)]));
    const next = moveClips(project, "seq", ["a"], seconds(6));
    expect(next.sequences[0]!.videoTracks[0]!.clips[0]).toMatchObject({ timelineIn: seconds(5), timelineOut: seconds(10) });
  });

  it("ripple-deletes and closes the gap", () => {
    const project = wrap(sequence([clip("a", 0, 5), clip("b", 5, 8), clip("c", 8, 12)]));
    const next = rippleDelete(project, "seq", ["a", "b"]);
    expect(next.sequences[0]!.videoTracks[0]!.clips).toEqual([
      expect.objectContaining({ id: "c", timelineIn: 0, timelineOut: seconds(4) }),
    ]);
  });

  it("overwrites the middle of a clip", () => {
    const project = wrap(sequence([clip("a", 0, 10)]));
    const next = overwrite(project, {
      sequenceId: "seq",
      name: "new",
      mediaId: "media-1",
      kind: "media",
      timelineIn: seconds(2),
      durationTicks: seconds(3),
      sourceIn: 0,
      sourceOut: seconds(3),
      labelColor: 2,
      video: true,
      audio: false,
    });
    const clips = next.sequences[0]!.videoTracks[0]!.clips;
    expect(clips.map((item) => [item.name, item.timelineIn, item.timelineOut])).toEqual([
      ["a", 0, seconds(2)],
      ["new", seconds(2), seconds(5)],
      ["a", seconds(5), seconds(10)],
    ]);
    expect(clips[1]!.mediaId).toBe("media-1");
  });

  it("uses one second of ticks", () => {
    expect(seconds(1)).toBe(TICKS_PER_SECOND);
  });
});
