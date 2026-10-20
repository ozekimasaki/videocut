import type { BinNode, Clip, ClipKind, MediaRef, Project, Sequence, Track } from "@videocut/model";
import { newId } from "../model/ids";
import { ticksPerFrame } from "../model/timecode";
import type { EditCommand } from "./command-stack";

export interface EditCommandResult {
  project: Project;
  changed: boolean;
}

function command(label: string, apply: (project: Project) => Project): EditCommand {
  return { label, apply };
}

function findSequence(project: Project, sequenceId: string): Sequence | undefined {
  return project.sequences.find((sequence) => sequence.id === sequenceId);
}

function mapSequence(project: Project, sequenceId: string, map: (sequence: Sequence) => Sequence): Project {
  return {
    ...project,
    sequences: project.sequences.map((sequence) => (sequence.id === sequenceId ? map(sequence) : sequence)),
  };
}

function allTracks(sequence: Sequence): Track[] {
  return [...sequence.videoTracks, ...sequence.audioTracks];
}

function mapAllTracks(sequence: Sequence, map: (track: Track) => Track): Sequence {
  return {
    ...sequence,
    videoTracks: sequence.videoTracks.map(map),
    audioTracks: sequence.audioTracks.map(map),
  };
}

function minTicks(sequence: Sequence): number {
  return ticksPerFrame(sequence.settings.fps);
}

function sortedClips(track: Track): Clip[] {
  return [...track.clips].sort((left, right) => left.timelineIn - right.timelineIn);
}

function shiftClip(clip: Clip, delta: number): Clip {
  return { ...clip, timelineIn: clip.timelineIn + delta, timelineOut: clip.timelineOut + delta };
}

function sourceDelta(clip: Clip, timelineDelta: number): number {
  return Math.round(timelineDelta * clip.speed);
}

function withSourceIn(clip: Clip, timelineDelta: number): Clip {
  const delta = sourceDelta(clip, timelineDelta);
  if (clip.reversed) return { ...clip, sourceOut: clip.sourceOut - delta };
  return { ...clip, sourceIn: clip.sourceIn + delta };
}

function withSourceOut(clip: Clip, timelineDelta: number): Clip {
  const delta = sourceDelta(clip, timelineDelta);
  if (clip.reversed) return { ...clip, sourceIn: clip.sourceIn + delta };
  return { ...clip, sourceOut: clip.sourceOut - delta };
}

function rangeOk(clip: Clip, minimum: number): boolean {
  return clip.timelineOut - clip.timelineIn >= minimum && clip.sourceOut > clip.sourceIn && clip.sourceIn >= 0 && clip.sourceOut >= 0;
}

function mediaDuration(project: Project, clip: Clip): number | null {
  if (!clip.mediaId) return null;
  const media = project.media.find((item) => item.id === clip.mediaId);
  if (!media || media.durationTicks <= 0) return null;
  return media.durationTicks;
}

function withinMedia(project: Project, original: Clip, next: Clip): boolean {
  const duration = mediaDuration(project, original);
  if (duration == null) return true;
  return next.sourceIn >= 0 && next.sourceOut <= duration;
}

export function razorAtPlayhead(project: Project, sequenceId: string, ticks: number): Project {
  const sequence = findSequence(project, sequenceId);
  if (!sequence) return project;
  const tracks = allTracks(sequence);
  const located = new Map<string, { track: Track; clip: Clip }>();
  for (const track of tracks) {
    for (const clip of track.clips) located.set(clip.id, { track, clip });
  }
  const ids = new Set<string>();
  for (const track of tracks) {
    if (!track.targeted || track.locked) continue;
    for (const clip of track.clips) {
      if (ticks <= clip.timelineIn || ticks >= clip.timelineOut) continue;
      ids.add(clip.id);
      for (const linked of clip.linkedIds ?? []) {
        const other = located.get(linked);
        if (!other || other.track.locked) continue;
        if (ticks > other.clip.timelineIn && ticks < other.clip.timelineOut) ids.add(linked);
      }
    }
  }
  if (ids.size === 0) return project;
  const pieces = new Map<string, [Clip, Clip]>();
  const rightIds = new Map<string, string>();
  for (const id of ids) {
    const clip = located.get(id)?.clip;
    if (!clip) continue;
    const pair = splitClip(clip, ticks);
    if (!pair) continue;
    pieces.set(id, pair);
    rightIds.set(id, pair[1].id);
  }
  if (pieces.size === 0) return project;
  const relink = (clip: Clip, right: boolean): Clip => {
    if (!clip.linkedIds?.length) return clip;
    return {
      ...clip,
      linkedIds: clip.linkedIds.map((id) => (right && rightIds.has(id) ? rightIds.get(id)! : id)),
    };
  };
  return mapSequence(project, sequenceId, (current) =>
    mapAllTracks(current, (track) => ({
      ...track,
      clips: track.clips.flatMap((clip) => {
        const pair = pieces.get(clip.id);
        if (!pair) return [clip];
        return [relink(pair[0], false), relink(pair[1], true)];
      }),
    })),
  );
}

function splitClip(clip: Clip, ticks: number): [Clip, Clip] | null {
  if (ticks <= clip.timelineIn || ticks >= clip.timelineOut) return null;
  const delta = sourceDelta(clip, ticks - clip.timelineIn);
  if (clip.reversed) {
    const splitSource = clip.sourceOut - delta;
    return [
      { ...clip, timelineOut: ticks, sourceIn: splitSource },
      { ...clip, id: newId("clip"), timelineIn: ticks, sourceOut: splitSource },
    ];
  }
  const splitSource = clip.sourceIn + delta;
  return [
    { ...clip, timelineOut: ticks, sourceOut: splitSource },
    { ...clip, id: newId("clip"), timelineIn: ticks, sourceIn: splitSource },
  ];
}

export function rippleTrimPrevious(project: Project, sequenceId: string, ticks: number): Project {
  const sequence = findSequence(project, sequenceId);
  if (!sequence) return project;
  const minimum = minTicks(sequence);
  return mapSequence(project, sequenceId, (current) =>
    mapAllTracks(current, (track) => {
      if (!track.targeted || track.locked) return track;
      const clip = track.clips.find((item) => item.timelineIn < ticks && ticks < item.timelineOut);
      if (!clip) return track;
      return rippleHead(project, track, clip, ticks - clip.timelineIn, minimum);
    }),
  );
}

export function rippleTrimNext(project: Project, sequenceId: string, ticks: number): Project {
  const sequence = findSequence(project, sequenceId);
  if (!sequence) return project;
  const minimum = minTicks(sequence);
  return mapSequence(project, sequenceId, (current) =>
    mapAllTracks(current, (track) => {
      if (!track.targeted || track.locked) return track;
      const clip = track.clips.find((item) => item.timelineIn < ticks && ticks < item.timelineOut);
      if (!clip) return track;
      return rippleTail(project, track, clip, clip.timelineOut - ticks, minimum);
    }),
  );
}

function rippleHead(project: Project, track: Track, clip: Clip, delta: number, minimum: number): Track {
  if (delta === 0) return track;
  if (clip.timelineOut - clip.timelineIn - delta < minimum) return track;
  const trimmed: Clip = { ...withSourceIn(clip, delta), timelineOut: clip.timelineOut - delta };
  if (!rangeOk(trimmed, minimum) || !withinMedia(project, clip, trimmed)) return track;
  return {
    ...track,
    clips: track.clips.map((item) => {
      if (item.id === clip.id) return trimmed;
      if (item.timelineIn >= clip.timelineOut) return shiftClip(item, -delta);
      return item;
    }),
  };
}

function rippleTail(project: Project, track: Track, clip: Clip, removed: number, minimum: number): Track {
  if (removed === 0) return track;
  const nextOut = clip.timelineOut - removed;
  if (nextOut - clip.timelineIn < minimum) return track;
  const trimmed: Clip = { ...withSourceOut(clip, removed), timelineOut: nextOut };
  if (!rangeOk(trimmed, minimum) || !withinMedia(project, clip, trimmed)) return track;
  return {
    ...track,
    clips: track.clips.map((item) => {
      if (item.id === clip.id) return trimmed;
      if (item.timelineIn >= clip.timelineOut) return shiftClip(item, -removed);
      return item;
    }),
  };
}

export function rippleTrimEdge(
  project: Project,
  sequenceId: string,
  clipId: string,
  edge: "start" | "end",
  delta: number,
): Project {
  const sequence = findSequence(project, sequenceId);
  if (!sequence || delta === 0) return project;
  const minimum = minTicks(sequence);
  return mapSequence(project, sequenceId, (current) =>
    mapAllTracks(current, (track) => {
      if (track.locked) return track;
      const clip = track.clips.find((item) => item.id === clipId);
      if (!clip) return track;
      if (edge === "start") return rippleHead(project, track, clip, delta, minimum);
      return rippleTail(project, track, clip, -delta, minimum);
    }),
  );
}

export function trimEdge(
  project: Project,
  sequenceId: string,
  clipId: string,
  edge: "start" | "end",
  time: number,
): Project {
  const sequence = findSequence(project, sequenceId);
  if (!sequence) return project;
  const minimum = minTicks(sequence);
  return mapSequence(project, sequenceId, (current) =>
    mapAllTracks(current, (track) => {
      if (track.locked) return track;
      const clip = track.clips.find((item) => item.id === clipId);
      if (!clip) return track;
      const neighbors = sortedClips(track);
      const index = neighbors.findIndex((item) => item.id === clipId);
      if (edge === "start") {
        const previous = index > 0 ? neighbors[index - 1] : undefined;
        const limit = previous && previous.timelineOut <= clip.timelineIn ? previous.timelineOut : 0;
        const nextIn = Math.max(limit, Math.min(time, clip.timelineOut - minimum));
        const delta = nextIn - clip.timelineIn;
        const trimmed: Clip = { ...withSourceIn(clip, delta), timelineIn: nextIn };
        if (!rangeOk(trimmed, minimum) || !withinMedia(project, clip, trimmed)) return track;
        return { ...track, clips: track.clips.map((item) => (item.id === clip.id ? trimmed : item)) };
      }
      const next = neighbors[index + 1];
      const limit = next && next.timelineIn >= clip.timelineOut ? next.timelineIn : Number.POSITIVE_INFINITY;
      const nextOut = Math.min(limit, Math.max(time, clip.timelineIn + minimum));
      const removed = clip.timelineOut - nextOut;
      const trimmed: Clip = { ...withSourceOut(clip, removed), timelineOut: nextOut };
      if (!rangeOk(trimmed, minimum) || !withinMedia(project, clip, trimmed)) return track;
      return { ...track, clips: track.clips.map((item) => (item.id === clip.id ? trimmed : item)) };
    }),
  );
}

export function rollEdit(
  project: Project,
  sequenceId: string,
  clipId: string,
  edge: "start" | "end",
  time: number,
): Project {
  const sequence = findSequence(project, sequenceId);
  if (!sequence) return project;
  const minimum = minTicks(sequence);
  return mapSequence(project, sequenceId, (current) =>
    mapAllTracks(current, (track) => {
      if (track.locked) return track;
      const clips = sortedClips(track);
      const index = clips.findIndex((item) => item.id === clipId);
      if (index < 0) return track;
      const clip = clips[index]!;
      const left = edge === "end" ? clip : clips[index - 1];
      const right = edge === "end" ? clips[index + 1] : clip;
      if (!left || !right || left.timelineOut !== right.timelineIn) return track;
      const cut = left.timelineOut;
      const nextCut = Math.max(left.timelineIn + minimum, Math.min(time, right.timelineOut - minimum));
      const delta = nextCut - cut;
      if (delta === 0) return track;
      const nextLeft: Clip = { ...withSourceOut(left, -delta), timelineOut: nextCut };
      const nextRight: Clip = { ...withSourceIn(right, delta), timelineIn: nextCut };
      if (!rangeOk(nextLeft, minimum) || !rangeOk(nextRight, minimum)) return track;
      if (!withinMedia(project, left, nextLeft) || !withinMedia(project, right, nextRight)) return track;
      return {
        ...track,
        clips: track.clips.map((item) => {
          if (item.id === left.id) return nextLeft;
          if (item.id === right.id) return nextRight;
          return item;
        }),
      };
    }),
  );
}

export function slipClip(project: Project, sequenceId: string, clipId: string, delta: number): Project {
  if (delta === 0) return project;
  const sequence = findSequence(project, sequenceId);
  if (!sequence) return project;
  const minimum = minTicks(sequence);
  return mapSequence(project, sequenceId, (current) =>
    mapAllTracks(current, (track) => ({
      ...track,
      clips: track.clips.map((clip) => {
        if (clip.id !== clipId || track.locked) return clip;
        const next: Clip = {
          ...clip,
          sourceIn: clip.sourceIn + delta,
          sourceOut: clip.sourceOut + delta,
        };
        if (!rangeOk(next, minimum) || !withinMedia(project, clip, next)) return clip;
        return next;
      }),
    })),
  );
}

export function slideClip(project: Project, sequenceId: string, clipId: string, delta: number): Project {
  if (delta === 0) return project;
  const sequence = findSequence(project, sequenceId);
  if (!sequence) return project;
  const minimum = minTicks(sequence);
  return mapSequence(project, sequenceId, (current) =>
    mapAllTracks(current, (track) => {
      if (track.locked) return track;
      const clips = sortedClips(track);
      const index = clips.findIndex((item) => item.id === clipId);
      if (index <= 0 || index >= clips.length - 1) return track;
      const previous = clips[index - 1]!;
      const clip = clips[index]!;
      const next = clips[index + 1]!;
      if (previous.timelineOut !== clip.timelineIn || clip.timelineOut !== next.timelineIn) return track;
      const maxRight = next.timelineOut - next.timelineIn - minimum;
      const maxLeft = previous.timelineOut - previous.timelineIn - minimum;
      const applied = Math.max(-maxLeft, Math.min(delta, maxRight));
      if (applied === 0) return track;
      const nextPrevious: Clip = { ...withSourceOut(previous, -applied), timelineOut: previous.timelineOut + applied };
      const moved = shiftClip(clip, applied);
      const nextFollowing: Clip = { ...withSourceIn(next, applied), timelineIn: next.timelineIn + applied };
      if (!rangeOk(nextPrevious, minimum) || !rangeOk(nextFollowing, minimum) || !rangeOk(moved, minimum)) return track;
      if (!withinMedia(project, previous, nextPrevious) || !withinMedia(project, next, nextFollowing)) return track;
      return {
        ...track,
        clips: track.clips.map((item) => {
          if (item.id === previous.id) return nextPrevious;
          if (item.id === clip.id) return moved;
          if (item.id === next.id) return nextFollowing;
          return item;
        }),
      };
    }),
  );
}

export function moveClips(project: Project, sequenceId: string, clipIds: readonly string[], delta: number): Project {
  if (delta === 0 || clipIds.length === 0) return project;
  const sequence = findSequence(project, sequenceId);
  if (!sequence) return project;
  const ids = expandLinked(sequence, clipIds);
  let applied = delta;
  for (const track of allTracks(sequence)) {
    if (track.locked) continue;
    for (const clip of track.clips) {
      if (!ids.has(clip.id)) continue;
      applied = clampMove(track, clip, ids, applied);
    }
  }
  if (applied === 0) return project;
  return mapSequence(project, sequenceId, (current) =>
    mapAllTracks(current, (track) => {
      if (track.locked) return track;
      return {
        ...track,
        clips: track.clips.map((clip) => (ids.has(clip.id) ? shiftClip(clip, applied) : clip)),
      };
    }),
  );
}

function clampMove(track: Track, clip: Clip, ids: Set<string>, delta: number): number {
  let next = delta;
  if (clip.timelineIn + next < 0) next = -clip.timelineIn;
  for (const other of sortedClips(track)) {
    if (ids.has(other.id)) continue;
    const start = clip.timelineIn + next;
    const end = clip.timelineOut + next;
    if (end > other.timelineIn && start < other.timelineOut) {
      next = next > 0 ? other.timelineIn - clip.timelineOut : other.timelineOut - clip.timelineIn;
    }
  }
  if (clip.timelineIn + next < 0) next = -clip.timelineIn;
  return next;
}

function expandLinked(sequence: Sequence, clipIds: readonly string[]): Set<string> {
  const ids = new Set(clipIds);
  const byId = new Map<string, Clip>();
  for (const track of allTracks(sequence)) {
    for (const clip of track.clips) byId.set(clip.id, clip);
  }
  for (const id of [...ids]) {
    for (const linked of byId.get(id)?.linkedIds ?? []) ids.add(linked);
  }
  return ids;
}

export function rippleDelete(project: Project, sequenceId: string, clipIds: readonly string[]): Project {
  const sequence = findSequence(project, sequenceId);
  if (!sequence || clipIds.length === 0) return project;
  const ids = expandLinked(sequence, clipIds);
  return mapSequence(project, sequenceId, (current) =>
    mapAllTracks(current, (track) => {
      if (track.locked) return track;
      let shift = 0;
      const clips: Clip[] = [];
      for (const clip of sortedClips(track)) {
        if (ids.has(clip.id)) {
          shift -= clip.timelineOut - clip.timelineIn;
          continue;
        }
        clips.push(shift === 0 ? clip : shiftClip(clip, shift));
      }
      return { ...track, clips };
    }),
  );
}

export interface OverwriteRequest {
  sequenceId: string;
  name: string;
  mediaId?: string;
  kind: ClipKind;
  timelineIn: number;
  durationTicks: number;
  sourceIn: number;
  sourceOut: number;
  labelColor: number;
  video: boolean;
  audio: boolean;
}

export function overwrite(project: Project, request: OverwriteRequest): Project {
  const sequence = findSequence(project, request.sequenceId);
  if (!sequence || request.durationTicks <= 0) return project;
  const minimum = minTicks(sequence);
  if (request.durationTicks < minimum) return project;
  const start = Math.max(0, request.timelineIn);
  const end = start + request.durationTicks;
  const video = request.video ? firstOpenTrack(sequence.videoTracks) : undefined;
  const audio = request.audio ? firstOpenTrack(sequence.audioTracks) : undefined;
  if (!video && !audio) return project;
  const videoId = video ? newId("clip") : undefined;
  const audioId = audio ? newId("clip") : undefined;
  const base = {
    name: request.name,
    kind: request.kind,
    timelineIn: start,
    timelineOut: end,
    sourceIn: request.sourceIn,
    sourceOut: request.sourceOut,
    speed: 1,
    reversed: false,
    enabled: true,
    labelColor: request.labelColor,
    ...(request.mediaId ? { mediaId: request.mediaId } : {}),
  };
  return mapSequence(project, request.sequenceId, (current) => ({
    ...current,
    videoTracks: videoId
      ? current.videoTracks.map((track) =>
          track.id === video?.id
            ? placeClip(track, {
                ...base,
                id: videoId,
                ...(audioId ? { linkedIds: [audioId] } : {}),
              })
            : track,
        )
      : current.videoTracks,
    audioTracks: audioId
      ? current.audioTracks.map((track) =>
          track.id === audio?.id
            ? placeClip(track, {
                ...base,
                id: audioId,
                kind: "media",
                ...(videoId ? { linkedIds: [videoId] } : {}),
              })
            : track,
        )
      : current.audioTracks,
  }));
}

function firstOpenTrack(tracks: Track[]): Track | undefined {
  return [...tracks].filter((track) => track.targeted && !track.locked).sort((left, right) => left.index - right.index)[0];
}

function placeClip(track: Track, incoming: Clip): Track {
  const clips: Clip[] = [];
  for (const clip of track.clips) {
    if (clip.timelineOut <= incoming.timelineIn || clip.timelineIn >= incoming.timelineOut) {
      clips.push(clip);
      continue;
    }
    if (clip.timelineIn < incoming.timelineIn) {
      const removed = clip.timelineOut - incoming.timelineIn;
      clips.push({ ...withSourceOut(clip, removed), timelineOut: incoming.timelineIn });
    }
    if (clip.timelineOut > incoming.timelineOut) {
      const removed = incoming.timelineOut - clip.timelineIn;
      clips.push({ ...withSourceIn(clip, removed), timelineIn: incoming.timelineOut });
    }
  }
  clips.push(incoming);
  clips.sort((left, right) => left.timelineIn - right.timelineIn);
  return { ...track, clips };
}

export function addMarker(project: Project, sequenceId: string, ticks: number, name: string): Project {
  return mapSequence(project, sequenceId, (sequence) => ({
    ...sequence,
    markers: [
      ...(sequence.markers ?? []),
      {
        id: newId("marker"),
        timeTicks: Math.max(0, ticks),
        durationTicks: 0,
        name,
        comment: "",
        color: 3,
        kind: "comment",
      },
    ],
  }));
}

export function importMedia(project: Project, files: readonly MediaRef[]): Project {
  if (files.length === 0) return project;
  const media = [...project.media];
  const imported: BinNode[] = [];
  for (const file of files) {
    const mediaId = file.id && !media.some((item) => item.id === file.id) ? file.id : newId("media");
    media.push({
      id: mediaId,
      name: file.name,
      path: file.path,
      durationTicks: file.durationTicks,
      ...(file.video ? { video: file.video } : {}),
      ...(file.audio ? { audio: file.audio } : {}),
      ...(file.proxyPath ? { proxyPath: file.proxyPath } : {}),
    });
    imported.push({ id: newId("bin"), name: file.name, kind: "clip", mediaId });
  }
  const bins = project.bins.length > 0 ? project.bins : [{ id: newId("bin"), name: "マスター", kind: "bin" as const, children: [] }];
  const [first, ...rest] = bins;
  if (!first) return { ...project, media };
  return {
    ...project,
    media,
    bins: [{ ...first, children: [...(first.children ?? []), ...imported] }, ...rest],
  };
}

export function renameProject(project: Project, name: string): Project {
  const trimmed = name.trim();
  if (!trimmed || trimmed === project.name) return project;
  return { ...project, name: trimmed };
}

export function renameBinItem(project: Project, itemId: string, name: string): Project {
  const trimmed = name.trim();
  if (!trimmed) return project;
  return { ...project, bins: mapBins(project.bins, itemId, trimmed) };
}

function mapBins(nodes: BinNode[], itemId: string, name: string): BinNode[] {
  return nodes.map((node) => {
    const children = node.children ? mapBins(node.children, itemId, name) : undefined;
    const renamed = node.id === itemId ? name : node.name;
    return children ? { ...node, name: renamed, children } : { ...node, name: renamed };
  });
}

export function addSequence(project: Project, name: string): Project {
  const sequence = {
    id: newId("seq"),
    name,
    settings: project.sequences[0]?.settings ?? {
      width: 1920,
      height: 1080,
      fps: { num: 30, den: 1 },
      sampleRate: 48000,
      audioChannels: 2,
      startTimecodeTicks: 0,
    },
    videoTracks: [
      {
        id: newId("track"),
        index: 0,
        kind: "video" as const,
        clips: [],
        locked: false,
        muted: false,
        hidden: false,
        syncLock: true,
        targeted: true,
      },
    ],
    audioTracks: [
      {
        id: newId("track"),
        index: 0,
        kind: "audio" as const,
        clips: [],
        locked: false,
        muted: false,
        hidden: false,
        syncLock: true,
        targeted: true,
      },
    ],
    markers: [],
    captions: [],
  };
  const item: BinNode = { id: newId("bin"), name, kind: "sequence", sequenceId: sequence.id };
  const bins = project.bins.length > 0 ? project.bins : [{ id: newId("bin"), name: "マスター", kind: "bin" as const, children: [] }];
  const [first, ...rest] = bins;
  return {
    ...project,
    sequences: [...project.sequences, sequence],
    bins: first ? [{ ...first, children: [...(first.children ?? []), item] }, ...rest] : bins,
  };
}

export function patchTrack(
  project: Project,
  sequenceId: string,
  trackId: string,
  patch: Partial<Pick<Track, "targeted" | "locked" | "muted" | "hidden" | "syncLock">>,
): Project {
  return mapSequence(project, sequenceId, (sequence) =>
    mapAllTracks(sequence, (track) => (track.id === trackId ? { ...track, ...patch } : track)),
  );
}

export function toggleEffect(project: Project, sequenceId: string, clipId: string, effectInstanceId: string, enabled: boolean): Project {
  return mapSequence(project, sequenceId, (sequence) =>
    mapAllTracks(sequence, (track) => ({
      ...track,
      clips: track.clips.map((clip) => {
        if (clip.id !== clipId) return clip;
        return {
          ...clip,
          effects: (clip.effects ?? []).map((effect) => (effect.id === effectInstanceId ? { ...effect, enabled } : effect)),
        };
      }),
    })),
  );
}

export function setCaptionText(project: Project, sequenceId: string, captionId: string, text: string): Project {
  return mapSequence(project, sequenceId, (sequence) => ({
    ...sequence,
    captions: (sequence.captions ?? []).map((caption) => (caption.id === captionId ? { ...caption, text } : caption)),
  }));
}

export const commands = {
  razor: (sequenceId: string, ticks: number) => command("レーザー", (project) => razorAtPlayhead(project, sequenceId, ticks)),
  trimPrevious: (sequenceId: string, ticks: number) =>
    command("前の編集点をリップル", (project) => rippleTrimPrevious(project, sequenceId, ticks)),
  trimNext: (sequenceId: string, ticks: number) =>
    command("次の編集点をリップル", (project) => rippleTrimNext(project, sequenceId, ticks)),
  rippleEdge: (sequenceId: string, clipId: string, edge: "start" | "end", delta: number) =>
    command("リップルトリム", (project) => rippleTrimEdge(project, sequenceId, clipId, edge, delta)),
  trim: (sequenceId: string, clipId: string, edge: "start" | "end", time: number) =>
    command("トリム", (project) => trimEdge(project, sequenceId, clipId, edge, time)),
  roll: (sequenceId: string, clipId: string, edge: "start" | "end", time: number) =>
    command("ローリング", (project) => rollEdit(project, sequenceId, clipId, edge, time)),
  slip: (sequenceId: string, clipId: string, delta: number) =>
    command("スリップ", (project) => slipClip(project, sequenceId, clipId, delta)),
  slide: (sequenceId: string, clipId: string, delta: number) =>
    command("スライド", (project) => slideClip(project, sequenceId, clipId, delta)),
  move: (sequenceId: string, clipIds: readonly string[], delta: number) =>
    command("移動", (project) => moveClips(project, sequenceId, clipIds, delta)),
  rippleDelete: (sequenceId: string, clipIds: readonly string[]) =>
    command("リップル削除", (project) => rippleDelete(project, sequenceId, clipIds)),
  overwrite: (request: OverwriteRequest) => command("上書き", (project) => overwrite(project, request)),
  marker: (sequenceId: string, ticks: number, name = "マーカー") =>
    command("マーカー", (project) => addMarker(project, sequenceId, ticks, name)),
  importMedia: (files: readonly MediaRef[]) => command("読み込み", (project) => importMedia(project, files)),
  renameProject: (name: string) => command("プロジェクト名", (project) => renameProject(project, name)),
  renameBin: (itemId: string, name: string) => command("名前の変更", (project) => renameBinItem(project, itemId, name)),
  addSequence: (name: string) => command("シーケンス", (project) => addSequence(project, name)),
  patchTrack: (sequenceId: string, trackId: string, patch: Partial<Pick<Track, "targeted" | "locked" | "muted" | "hidden" | "syncLock">>) =>
    command("トラック", (project) => patchTrack(project, sequenceId, trackId, patch)),
  toggleEffect: (sequenceId: string, clipId: string, effectInstanceId: string, enabled: boolean) =>
    command("エフェクト", (project) => toggleEffect(project, sequenceId, clipId, effectInstanceId, enabled)),
  caption: (sequenceId: string, captionId: string, text: string) =>
    command("キャプション", (project) => setCaptionText(project, sequenceId, captionId, text)),
};
