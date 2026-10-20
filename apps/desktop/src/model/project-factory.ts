import {
  effectIds,
  TICKS_PER_SECOND,
  type BinNode,
  type Clip,
  type Project,
  type Sequence,
  type SequenceSettings,
  type Track,
} from "@videocut/model";
import { newId } from "./ids";

export const HD1080_30: SequenceSettings = {
  width: 1920,
  height: 1080,
  fps: { num: 30, den: 1 },
  sampleRate: 48000,
  audioChannels: 2,
  startTimecodeTicks: 0,
};

export function emptyTrack(index: number, kind: Track["kind"]): Track {
  return {
    id: newId("track"),
    index,
    kind,
    clips: [],
    transitions: [],
    locked: false,
    muted: false,
    hidden: false,
    syncLock: true,
    targeted: true,
  };
}

export function makeClip(partial: Partial<Clip> & Pick<Clip, "name" | "kind" | "timelineIn" | "timelineOut">): Clip {
  return {
    id: partial.id ?? newId("clip"),
    name: partial.name,
    kind: partial.kind,
    timelineIn: partial.timelineIn,
    timelineOut: partial.timelineOut,
    sourceIn: partial.sourceIn ?? 0,
    sourceOut: partial.sourceOut ?? partial.timelineOut - partial.timelineIn,
    speed: partial.speed ?? 1,
    reversed: partial.reversed ?? false,
    enabled: partial.enabled ?? true,
    labelColor: partial.labelColor ?? 0,
    ...(partial.mediaId ? { mediaId: partial.mediaId } : {}),
    ...(partial.nestedSequenceId ? { nestedSequenceId: partial.nestedSequenceId } : {}),
    ...(partial.linkedIds ? { linkedIds: partial.linkedIds } : {}),
    ...(partial.effects ? { effects: partial.effects } : {}),
  };
}

export function makeSequence(name: string, settings: SequenceSettings = HD1080_30): Sequence {
  return {
    id: newId("seq"),
    name,
    settings,
    videoTracks: [emptyTrack(0, "video")],
    audioTracks: [emptyTrack(0, "audio")],
    markers: [],
    captions: [],
  };
}

export function createEmptyProject(name = "無題"): Project {
  const sequence = makeSequence("シーケンス 01");
  return {
    id: newId("proj"),
    name,
    bins: [binWithSequence(sequence)],
    sequences: [sequence],
    media: [],
  };
}

export function createDemoProject(): Project {
  const sequence = makeSequence("シーケンス 01");
  const five = TICKS_PER_SECOND * 5;
  const titleEnd = TICKS_PER_SECOND * 3;
  const title = makeClip({
    name: "タイトル",
    kind: "title",
    timelineIn: 0,
    timelineOut: titleEnd,
    sourceIn: 0,
    sourceOut: titleEnd,
    labelColor: 5,
    effects: [
      {
        id: newId("fx"),
        effectId: effectIds.opacity,
        enabled: true,
        params: [{ id: newId("param"), value: { type: "float", value: 1 } }],
      },
    ],
  });
  const colorA = makeClip({
    name: "カラー A",
    kind: "colorMatte",
    timelineIn: 0,
    timelineOut: five,
    sourceIn: 0,
    sourceOut: five,
    labelColor: 1,
  });
  const colorB = makeClip({
    name: "カラー B",
    kind: "colorMatte",
    timelineIn: five,
    timelineOut: five * 2,
    sourceIn: 0,
    sourceOut: five,
    labelColor: 4,
  });
  const upper = emptyTrack(1, "video");
  upper.clips = [title];
  const lower = emptyTrack(0, "video");
  lower.clips = [colorA, colorB];
  sequence.videoTracks = [lower, upper];
  sequence.markers = [
    {
      id: newId("marker"),
      timeTicks: five,
      durationTicks: 0,
      name: "カット",
      comment: "",
      color: 3,
      kind: "comment",
    },
  ];
  sequence.captions = [
    {
      id: newId("caption"),
      startTicks: 0,
      endTicks: titleEnd,
      text: "VideoCut",
    },
  ];
  return {
    id: newId("proj"),
    name: "無題",
    bins: [binWithSequence(sequence)],
    sequences: [sequence],
    media: [],
  };
}

function binWithSequence(sequence: Sequence): BinNode {
  return {
    id: newId("bin"),
    name: "マスター",
    kind: "bin",
    children: [
      {
        id: newId("bin"),
        name: sequence.name,
        kind: "sequence",
        sequenceId: sequence.id,
      },
    ],
  };
}

export function isProject(value: unknown): value is Project {
  if (!value || typeof value !== "object") return false;
  const record = value as Partial<Project>;
  return (
    typeof record.id === "string" &&
    typeof record.name === "string" &&
    Array.isArray(record.bins) &&
    Array.isArray(record.sequences) &&
    Array.isArray(record.media)
  );
}
