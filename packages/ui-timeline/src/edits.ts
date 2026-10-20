/**
 * Sequence edit engine used by the VideoCut timeline.
 * Time is ticks. Failed edits throw and leave the sequence unchanged.
 * Linked clips share timeline edits. Targeted tracks receive insert,
 * overwrite, lift, and extract. Sync lock ripples the other tracks.
 */

import {
  TICKS_PER_SECOND,
  type Clip,
  type Marker,
  type MarkerKind,
  type Sequence,
  type Track,
  type TrackKind,
} from "@videocut/model";

export { TICKS_PER_SECOND };

export type EditErrorCode =
  | "track-not-found"
  | "clip-not-found"
  | "marker-not-found"
  | "angle-not-found"
  | "track-locked"
  | "track-not-targeted"
  | "empty-edit"
  | "invalid-range"
  | "invalid-time"
  | "zero-duration"
  | "insufficient-source"
  | "not-adjacent"
  | "overlap"
  | "invalid-speed"
  | "not-under-playhead"
  | "duplicate-track";

export class EditError extends Error {
  readonly code: EditErrorCode;

  constructor(code: EditErrorCode, message: string) {
    super(message);
    this.name = "EditError";
    this.code = code;
  }
}

export type TrimSide = "in" | "out";

export interface Placement {
  trackId: string;
  clip: Clip;
}

export interface MarkerDraft {
  timeTicks: number;
  durationTicks: number;
  name: string;
  comment: string;
  color: number;
  kind: MarkerKind;
}

export type SnapKind = "sequence-start" | "clip-in" | "clip-out" | "marker" | "playhead";

export interface SnapHit {
  ticks: number;
  kind: SnapKind;
  distance: number;
}

interface RangeCut {
  removed: Clip[];
  splitPairs: Array<[string, string]>;
}

function newId(prefix: string): string {
  return `${prefix}-${crypto.randomUUID()}`;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function fail(code: EditErrorCode, message: string): never {
  throw new EditError(code, message);
}

export function sourceAt(clip: Clip, timelineTicks: number): number {
  const elapsed = Math.max(0, timelineTicks - clip.timelineIn);
  const advanced = scaleBySpeed(elapsed, clip.speed);
  return clip.reversed
    ? clip.sourceOut - advanced
    : clip.sourceIn + advanced;
}

function scaleBySpeed(ticks: number, speed: number): number {
  if (ticks === 0 || speed === 0) {
    return 0;
  }
  return Math.round(ticks * speed);
}

function allTracks(seq: Sequence): Track[] {
  return [...seq.videoTracks, ...seq.audioTracks];
}

function trackRef(seq: Sequence, id: string): Track {
  const track = allTracks(seq).find((item) => item.id === id);
  if (!track) {
    fail("track-not-found", `track not found: ${id}`);
  }
  return track;
}

function clipRef(seq: Sequence, id: string): Clip {
  for (const track of allTracks(seq)) {
    const clip = track.clips.find((item) => item.id === id);
    if (clip) {
      return clip;
    }
  }
  fail("clip-not-found", `clip not found: ${id}`);
}

function trackIdOf(seq: Sequence, clipId: string): string {
  const track = allTracks(seq).find((item) => item.clips.some((clip) => clip.id === clipId));
  if (!track) {
    fail("clip-not-found", `clip not found: ${clipId}`);
  }
  return track.id;
}

function clipExists(seq: Sequence, id: string): boolean {
  return allTracks(seq).some((track) => track.clips.some((clip) => clip.id === id));
}

function linksOf(clip: Clip): string[] {
  return clip.linkedIds ?? [];
}

function sortClips(track: Track): void {
  track.clips.sort((a, b) => a.timelineIn - b.timelineIn || a.timelineOut - b.timelineOut || a.id.localeCompare(b.id));
}

function ensureNoOverlap(track: Track): void {
  const clips = [...track.clips].sort((a, b) => a.timelineIn - b.timelineIn || a.timelineOut - b.timelineOut);
  for (let index = 1; index < clips.length; index += 1) {
    const previous = clips[index - 1];
    const current = clips[index];
    if (previous && current && previous.timelineOut > current.timelineIn) {
      fail("overlap", "edit would overlap another clip");
    }
  }
}

function ensureSequence(seq: Sequence): void {
  for (const track of allTracks(seq)) {
    ensureNoOverlap(track);
    for (const clip of track.clips) {
      if (clip.timelineIn < 0) {
        fail("invalid-time", "time is outside the sequence");
      }
      if (clip.timelineOut <= clip.timelineIn) {
        fail("zero-duration", "clip duration must stay positive");
      }
    }
  }
}

function replaceSequence(target: Sequence, source: Sequence): void {
  target.name = source.name;
  target.settings = source.settings;
  target.videoTracks = source.videoTracks;
  target.audioTracks = source.audioTracks;
  target.markers = source.markers;
  target.captions = source.captions;
  target.multicamAngles = source.multicamAngles;
}

function commit<T>(seq: Sequence, edit: (draft: Sequence) => T): T {
  const draft = clone(seq);
  const value = edit(draft);
  ensureSequence(draft);
  replaceSequence(seq, draft);
  return value;
}

function shiftClip(clip: Clip, delta: number): void {
  if (delta === 0) {
    return;
  }
  const timelineIn = clip.timelineIn + delta;
  const timelineOut = clip.timelineOut + delta;
  if (!Number.isFinite(timelineIn) || !Number.isFinite(timelineOut) || timelineIn < 0 || timelineOut <= timelineIn) {
    fail("invalid-time", "time is outside the sequence");
  }
  clip.timelineIn = timelineIn;
  clip.timelineOut = timelineOut;
}

function splitOwned(clip: Clip, at: number): [Clip, Clip] {
  if (at <= clip.timelineIn || at >= clip.timelineOut) {
    fail("not-under-playhead", "playhead is not inside the clip");
  }
  const left = clone(clip);
  const right = clone(clip);
  right.id = newId("clip");
  right.linkedIds = [];
  left.timelineOut = at;
  right.timelineIn = at;
  const cut = sourceAt(left, at);
  if (left.reversed) {
    left.sourceIn = cut;
    right.sourceOut = cut;
  } else {
    left.sourceOut = cut;
    right.sourceIn = cut;
  }
  if (left.speed !== 0 && left.sourceOut <= left.sourceIn) {
    fail("insufficient-source", "not enough source media for this edit");
  }
  if (right.speed !== 0 && right.sourceOut <= right.sourceIn) {
    fail("insufficient-source", "not enough source media for this edit");
  }
  return [left, right];
}

function ignored(ignore: readonly string[], id: string): boolean {
  return ignore.includes(id);
}

function clearRange(track: Track, start: number, end: number, ignore: readonly string[]): RangeCut {
  if (end <= start) {
    fail("invalid-range", "range must be positive");
  }
  const original = track.clips;
  const kept: Clip[] = [];
  const removed: Clip[] = [];
  const splitPairs: Array<[string, string]> = [];
  for (const clip of original) {
    if (ignored(ignore, clip.id)) {
      kept.push(clip);
      continue;
    }
    if (clip.timelineOut <= start || clip.timelineIn >= end) {
      kept.push(clip);
      continue;
    }
    if (clip.timelineIn < start) {
      const [left, rest] = splitOwned(clip, start);
      kept.push(left);
      if (rest.timelineOut <= end) {
        removed.push(rest);
      } else {
        const [mid, right] = splitOwned(rest, end);
        splitPairs.push([left.id, right.id]);
        removed.push(mid);
        kept.push(right);
      }
    } else if (clip.timelineOut <= end) {
      removed.push(clip);
    } else {
      const [mid, right] = splitOwned(clip, end);
      const midId = mid.id;
      const rightId = right.id;
      mid.id = rightId;
      right.id = midId;
      const midLinks = mid.linkedIds;
      mid.linkedIds = right.linkedIds;
      right.linkedIds = midLinks;
      removed.push(mid);
      kept.push(right);
    }
  }
  kept.sort((a, b) => a.timelineIn - b.timelineIn);
  track.clips = kept;
  return { removed, splitPairs };
}

function shiftStartingAt(track: Track, point: number, delta: number, ignore: readonly string[]): void {
  if (delta === 0) {
    return;
  }
  for (const clip of track.clips) {
    if (clip.timelineIn >= point && !ignored(ignore, clip.id)) {
      shiftClip(clip, delta);
    }
  }
  sortClips(track);
}

function insertTime(track: Track, at: number, delta: number): Array<[string, string]> {
  if (delta <= 0) {
    fail("invalid-time", "time is outside the sequence");
  }
  const next: Clip[] = [];
  const pairs: Array<[string, string]> = [];
  for (const clip of track.clips) {
    if (clip.timelineIn < at && clip.timelineOut > at) {
      const [left, right] = splitOwned(clip, at);
      shiftClip(right, delta);
      pairs.push([left.id, right.id]);
      next.push(left, right);
    } else if (clip.timelineIn >= at) {
      shiftClip(clip, delta);
      next.push(clip);
    } else {
      next.push(clip);
    }
  }
  next.sort((a, b) => a.timelineIn - b.timelineIn);
  track.clips = next;
  return pairs;
}

function removeTime(track: Track, start: number, end: number): RangeCut {
  const cut = clearRange(track, start, end, []);
  shiftStartingAt(track, end, start - end, []);
  return cut;
}

function rippleFrom(track: Track, point: number, delta: number, ignore: readonly string[]): Array<[string, string]> {
  if (delta === 0) {
    return [];
  }
  if (delta > 0) {
    const pairs = splitContaining(track, point, ignore);
    shiftStartingAt(track, point, delta, ignore);
    return pairs;
  }
  const start = point + delta;
  if (start < 0) {
    fail("invalid-time", "time is outside the sequence");
  }
  const cut = clearRange(track, start, point, ignore);
  shiftStartingAt(track, point, delta, ignore);
  return cut.splitPairs;
}

function splitContaining(track: Track, at: number, ignore: readonly string[]): Array<[string, string]> {
  const next: Clip[] = [];
  const pairs: Array<[string, string]> = [];
  for (const clip of track.clips) {
    if (!ignored(ignore, clip.id) && clip.timelineIn < at && clip.timelineOut > at) {
      const [left, right] = splitOwned(clip, at);
      pairs.push([left.id, right.id]);
      next.push(left, right);
    } else {
      next.push(clip);
    }
  }
  track.clips = next;
  return pairs;
}

function splitClipOnTrack(track: Track, clipId: string, at: number): [string, string] {
  const index = track.clips.findIndex((clip) => clip.id === clipId);
  if (index < 0) {
    fail("clip-not-found", `clip not found: ${clipId}`);
  }
  const [clip] = track.clips.splice(index, 1);
  if (!clip) {
    fail("clip-not-found", `clip not found: ${clipId}`);
  }
  const [left, right] = splitOwned(clip, at);
  track.clips.push(left, right);
  sortClips(track);
  return [left.id, right.id];
}

function signedSource(deltaTimeline: number, speed: number): number {
  const magnitude = scaleBySpeed(Math.abs(deltaTimeline), speed);
  return deltaTimeline >= 0 ? magnitude : -magnitude;
}

function applyOutDelta(clip: Clip, deltaTimeline: number): void {
  if (deltaTimeline === 0 || clip.speed === 0) {
    return;
  }
  const signed = signedSource(deltaTimeline, clip.speed);
  if (clip.reversed) {
    const next = clip.sourceIn - signed;
    if (next < 0 || next >= clip.sourceOut) {
      fail("insufficient-source", "not enough source media for this edit");
    }
    clip.sourceIn = next;
  } else {
    const next = clip.sourceOut + signed;
    if (next <= clip.sourceIn) {
      fail("insufficient-source", "not enough source media for this edit");
    }
    clip.sourceOut = next;
  }
}

function applyInDelta(clip: Clip, deltaTimeline: number): void {
  if (deltaTimeline === 0 || clip.speed === 0) {
    return;
  }
  const signed = signedSource(deltaTimeline, clip.speed);
  if (clip.reversed) {
    const next = clip.sourceOut + signed;
    if (next <= clip.sourceIn) {
      fail("insufficient-source", "not enough source media for this edit");
    }
    clip.sourceOut = next;
  } else {
    const next = clip.sourceIn - signed;
    if (next < 0 || next >= clip.sourceOut) {
      fail("insufficient-source", "not enough source media for this edit");
    }
    clip.sourceIn = next;
  }
}

function timelineDurationOf(clip: Clip): number {
  const existing = clip.timelineOut - clip.timelineIn;
  if (existing > 0) {
    return existing;
  }
  if (clip.speed === 0) {
    fail("zero-duration", "clip duration must stay positive");
  }
  const source = clip.sourceOut - clip.sourceIn;
  if (source <= 0) {
    fail("zero-duration", "clip duration must stay positive");
  }
  const duration = Math.round(source / clip.speed);
  if (duration <= 0) {
    fail("zero-duration", "clip duration must stay positive");
  }
  return duration;
}

function placeClip(clip: Clip, at: number, seq: Sequence): number {
  if (at < 0) {
    fail("invalid-time", "time is outside the sequence");
  }
  if (!clip.id || clipExists(seq, clip.id)) {
    clip.id = newId("clip");
  }
  const duration = timelineDurationOf(clip);
  clip.timelineIn = at;
  clip.timelineOut = at + duration;
  return duration;
}

function expandLinks(seq: Sequence, seeds: readonly string[]): string[] {
  if (seeds.length === 0) {
    fail("empty-edit", "edit has no target");
  }
  const out: string[] = [];
  const stack = [...seeds];
  while (stack.length > 0) {
    const id = stack.pop();
    if (!id || out.includes(id)) {
      continue;
    }
    const links = linksOf(clipRef(seq, id));
    out.push(id);
    stack.push(...links);
  }
  return out;
}

function assignLinks(seq: Sequence, ids: readonly string[]): void {
  for (const id of ids) {
    clipRef(seq, id).linkedIds = ids.filter((other) => other !== id);
  }
}

function cleanupLinks(seq: Sequence): void {
  const existing = new Set(allTracks(seq).flatMap((track) => track.clips.map((clip) => clip.id)));
  for (const track of allTracks(seq)) {
    for (const clip of track.clips) {
      const unique = [...new Set(linksOf(clip).filter((id) => id !== clip.id && existing.has(id)))];
      unique.sort();
      clip.linkedIds = unique;
    }
  }
}

function relinkSplitRights(seq: Sequence, pairs: Array<[string, string]>): void {
  if (pairs.length < 2) {
    return;
  }
  const seen = new Set<string>();
  const groups: string[][] = [];
  for (const [left, right] of pairs) {
    if (seen.has(left)) {
      continue;
    }
    seen.add(left);
    const rights = [right];
    const stack = [left];
    const visited = new Set([left]);
    while (stack.length > 0) {
      const id = stack.pop();
      if (!id) {
        continue;
      }
      let partnerLinks: string[] = [];
      try {
        partnerLinks = linksOf(clipRef(seq, id));
      } catch (error) {
        if (!(error instanceof EditError)) {
          throw error;
        }
        partnerLinks = [];
      }
      for (const link of partnerLinks) {
        if (visited.has(link)) {
          continue;
        }
        visited.add(link);
        const partner = pairs.find(([partnerLeft]) => partnerLeft === link);
        if (partner) {
          rights.push(partner[1]);
          stack.push(link);
        }
      }
    }
    const unique = [...new Set(rights)];
    unique.sort();
    if (unique.length >= 2) {
      groups.push(unique);
    }
  }
  for (const group of groups) {
    assignLinks(seq, group);
  }
}

function requireTargetedOpen(seq: Sequence, trackId: string): void {
  const track = trackRef(seq, trackId);
  if (track.locked) {
    fail("track-locked", `track is locked: ${track.id}`);
  }
  if (!track.targeted) {
    fail("track-not-targeted", `track is not targeted: ${track.id}`);
  }
}

function targetedIds(seq: Sequence): string[] {
  const ids: string[] = [];
  for (const track of allTracks(seq)) {
    if (!track.targeted) {
      continue;
    }
    if (track.locked) {
      fail("track-locked", `track is locked: ${track.id}`);
    }
    ids.push(track.id);
  }
  if (ids.length === 0) {
    fail("empty-edit", "edit has no target");
  }
  return ids;
}

function rippleTrackIds(seq: Sequence, destinations: readonly string[]): string[] {
  const ids: string[] = [];
  for (const track of allTracks(seq)) {
    if (destinations.includes(track.id) || track.syncLock) {
      if (track.locked) {
        fail("track-locked", `track is locked: ${track.id}`);
      }
      ids.push(track.id);
    }
  }
  return ids;
}

export function setTrackTargeted(seq: Sequence, trackId: string, targeted: boolean): void {
  trackRef(seq, trackId).targeted = targeted;
}

export function setSyncLock(seq: Sequence, trackId: string, enabled: boolean): void {
  trackRef(seq, trackId).syncLock = enabled;
}

export function setTrackLocked(seq: Sequence, trackId: string, locked: boolean): void {
  trackRef(seq, trackId).locked = locked;
}

export function insert(seq: Sequence, at: number, placements: Placement[]): void {
  if (at < 0) {
    fail("invalid-time", "time is outside the sequence");
  }
  if (placements.length === 0) {
    fail("empty-edit", "edit has no target");
  }
  commit(seq, (draft) => {
    const seen = new Set<string>();
    const prepared: Placement[] = [];
    let ripple = 0;
    for (const placement of placements) {
      if (seen.has(placement.trackId)) {
        fail("duplicate-track", `track is already a destination of this edit: ${placement.trackId}`);
      }
      seen.add(placement.trackId);
      requireTargetedOpen(draft, placement.trackId);
      const clip = clone(placement.clip);
      ripple = Math.max(ripple, placeClip(clip, at, draft));
      prepared.push({ trackId: placement.trackId, clip });
    }
    const pairs: Array<[string, string]> = [];
    for (const trackId of rippleTrackIds(draft, prepared.map((item) => item.trackId))) {
      pairs.push(...insertTime(trackRef(draft, trackId), at, ripple));
    }
    relinkSplitRights(draft, pairs);
    for (const placement of prepared) {
      const track = trackRef(draft, placement.trackId);
      track.clips.push(placement.clip);
      sortClips(track);
    }
  });
}

export function overwrite(seq: Sequence, trackId: string, clip: Clip, at: number): void {
  if (at < 0) {
    fail("invalid-time", "time is outside the sequence");
  }
  commit(seq, (draft) => {
    requireTargetedOpen(draft, trackId);
    const placed = clone(clip);
    const duration = placeClip(placed, at, draft);
    const track = trackRef(draft, trackId);
    const cut = clearRange(track, at, at + duration, []);
    track.clips.push(placed);
    sortClips(track);
    cleanupLinks(draft);
    relinkSplitRights(draft, cut.splitPairs);
  });
}

export function lift(seq: Sequence, start: number, end: number): Clip[] {
  if (end <= start) {
    fail("invalid-range", "range must be positive");
  }
  return commit(seq, (draft) => {
    const removed: Clip[] = [];
    const pairs: Array<[string, string]> = [];
    for (const trackId of targetedIds(draft)) {
      const cut = clearRange(trackRef(draft, trackId), start, end, []);
      pairs.push(...cut.splitPairs);
      removed.push(...cut.removed);
    }
    cleanupLinks(draft);
    relinkSplitRights(draft, pairs);
    return removed;
  });
}

export function extract(seq: Sequence, start: number, end: number): Clip[] {
  if (end <= start) {
    fail("invalid-range", "range must be positive");
  }
  return commit(seq, (draft) => {
    const targeted = targetedIds(draft);
    const affect = [...targeted];
    for (const track of allTracks(draft)) {
      if (track.syncLock && !targeted.includes(track.id)) {
        if (track.locked) {
          fail("track-locked", `track is locked: ${track.id}`);
        }
        affect.push(track.id);
      }
    }
    const removed: Clip[] = [];
    const pairs: Array<[string, string]> = [];
    for (const trackId of affect) {
      const cut = removeTime(trackRef(draft, trackId), start, end);
      pairs.push(...cut.splitPairs);
      removed.push(...cut.removed);
    }
    cleanupLinks(draft);
    relinkSplitRights(draft, pairs);
    return removed;
  });
}

export function razor(seq: Sequence, at: number, trackIds?: readonly string[]): Array<[string, string]> {
  if (at < 0) {
    fail("invalid-time", "time is outside the sequence");
  }
  return commit(seq, (draft) => razorInner(draft, at, trackIds));
}

function razorInner(seq: Sequence, at: number, trackIds?: readonly string[]): Array<[string, string]> {
  const seeds: string[] = [];
  for (const track of allTracks(seq)) {
    if (trackIds && !trackIds.includes(track.id)) {
      continue;
    }
    if (track.locked) {
      continue;
    }
    for (const clip of track.clips) {
      if (clip.timelineIn < at && clip.timelineOut > at) {
        seeds.push(clip.id);
      }
    }
  }
  if (seeds.length === 0) {
    return [];
  }
  const pairs: Array<[string, string]> = [];
  for (const clipId of expandLinks(seq, seeds)) {
    const trackId = trackIdOf(seq, clipId);
    const track = trackRef(seq, trackId);
    if (track.locked) {
      fail("track-locked", `track is locked: ${trackId}`);
    }
    const contains = track.clips.some((clip) => clip.id === clipId && clip.timelineIn < at && clip.timelineOut > at);
    if (!contains) {
      continue;
    }
    pairs.push(splitClipOnTrack(track, clipId, at));
  }
  relinkSplitRights(seq, pairs);
  return pairs;
}

export function rippleTrim(seq: Sequence, clipId: string, side: TrimSide, deltaTicks: number): void {
  if (deltaTicks === 0) {
    return;
  }
  commit(seq, (draft) => {
    const ids = expandLinks(draft, [clipId]);
    const events: Array<[string, number, string, number]> = [];
    for (const id of ids) {
      const trackId = trackIdOf(draft, id);
      const track = trackRef(draft, trackId);
      if (track.locked) {
        fail("track-locked", `track is locked: ${trackId}`);
      }
      const clip = clipRef(draft, id);
      const newDuration = clip.timelineOut - clip.timelineIn + deltaTicks;
      if (newDuration <= 0) {
        fail("zero-duration", "clip duration must stay positive");
      }
      events.push([trackId, clip.timelineOut, id, newDuration]);
    }
    for (const [, , id, newDuration] of events) {
      const clip = clipRef(draft, id);
      switch (side) {
        case "in":
          applyInDelta(clip, deltaTicks);
          break;
        case "out":
          applyOutDelta(clip, deltaTicks);
          break;
        default: {
          const unexpected: never = side;
          fail("invalid-time", `unexpected trim side ${String(unexpected)}`);
        }
      }
      clip.timelineOut = clip.timelineIn + newDuration;
    }
    events.sort((a, b) => b[1] - a[1]);
    const seen = new Set<string>();
    const pairs: Array<[string, string]> = [];
    const handled = new Set<string>();
    for (const [trackId, oldOut] of events) {
      handled.add(trackId);
      const key = `${trackId}:${oldOut}`;
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      const ignore = events.filter((event) => event[0] === trackId).map((event) => event[2]);
      pairs.push(...rippleFrom(trackRef(draft, trackId), oldOut, deltaTicks, ignore));
    }
    const primary = events.find((event) => event[2] === clipId);
    if (!primary) {
      fail("clip-not-found", `clip not found: ${clipId}`);
    }
    for (const track of allTracks(draft)) {
      if (handled.has(track.id) || !track.syncLock) {
        continue;
      }
      if (track.locked) {
        fail("track-locked", `track is locked: ${track.id}`);
      }
      pairs.push(...rippleFrom(track, primary[1], deltaTicks, []));
    }
    relinkSplitRights(draft, pairs);
  });
}

export function rollTrim(seq: Sequence, leftClipId: string, deltaTicks: number): void {
  if (deltaTicks === 0) {
    return;
  }
  commit(seq, (draft) => {
    for (const id of expandLinks(draft, [leftClipId])) {
      const trackId = trackIdOf(draft, id);
      const track = trackRef(draft, trackId);
      if (track.locked) {
        fail("track-locked", `track is locked: ${trackId}`);
      }
      rollOnTrack(track, id, deltaTicks);
    }
  });
}

function rollOnTrack(track: Track, leftId: string, delta: number): void {
  const left = track.clips.find((clip) => clip.id === leftId);
  if (!left) {
    fail("clip-not-found", `clip not found: ${leftId}`);
  }
  const right = track.clips.find((clip) => clip.id !== leftId && clip.timelineIn === left.timelineOut);
  if (!right) {
    fail("not-adjacent", "clips must be adjacent");
  }
  const newLeft = left.timelineOut - left.timelineIn + delta;
  const newRight = right.timelineOut - right.timelineIn - delta;
  if (newLeft <= 0 || newRight <= 0) {
    fail("zero-duration", "clip duration must stay positive");
  }
  applyOutDelta(left, delta);
  applyInDelta(right, -delta);
  left.timelineOut = left.timelineIn + newLeft;
  right.timelineIn = right.timelineOut - newRight;
  if (right.timelineIn < 0) {
    fail("invalid-time", "time is outside the sequence");
  }
  sortClips(track);
}

export function slip(seq: Sequence, clipId: string, deltaSource: number): void {
  if (deltaSource === 0) {
    return;
  }
  commit(seq, (draft) => {
    for (const id of expandLinks(draft, [clipId])) {
      const clip = clipRef(draft, id);
      const sourceIn = clip.sourceIn + deltaSource;
      const sourceOut = clip.sourceOut + deltaSource;
      if (sourceIn < 0 || sourceOut < sourceIn || (clip.speed !== 0 && sourceOut <= sourceIn)) {
        fail("insufficient-source", "not enough source media for this edit");
      }
      clip.sourceIn = sourceIn;
      clip.sourceOut = sourceOut;
    }
  });
}

export function slide(seq: Sequence, clipId: string, deltaTicks: number): void {
  if (deltaTicks === 0) {
    return;
  }
  commit(seq, (draft) => {
    for (const id of expandLinks(draft, [clipId])) {
      const trackId = trackIdOf(draft, id);
      const track = trackRef(draft, trackId);
      if (track.locked) {
        fail("track-locked", `track is locked: ${trackId}`);
      }
      slideOnTrack(track, id, deltaTicks);
    }
  });
}

function slideOnTrack(track: Track, clipId: string, delta: number): void {
  const clip = track.clips.find((item) => item.id === clipId);
  if (!clip) {
    fail("clip-not-found", `clip not found: ${clipId}`);
  }
  const prev = track.clips.find((item) => item.id !== clipId && item.timelineOut === clip.timelineIn);
  const next = track.clips.find((item) => item.id !== clipId && item.timelineIn === clip.timelineOut);
  if (!prev || !next) {
    fail("not-adjacent", "clips must be adjacent");
  }
  const newPrev = prev.timelineOut - prev.timelineIn + delta;
  const newNext = next.timelineOut - next.timelineIn - delta;
  if (newPrev <= 0 || newNext <= 0) {
    fail("zero-duration", "clip duration must stay positive");
  }
  applyOutDelta(prev, delta);
  applyInDelta(next, -delta);
  prev.timelineOut = prev.timelineIn + newPrev;
  shiftClip(clip, delta);
  next.timelineIn = next.timelineOut - newNext;
  if (next.timelineIn < 0) {
    fail("invalid-time", "time is outside the sequence");
  }
  sortClips(track);
}

export function rippleDelete(seq: Sequence, clipIds: readonly string[]): void {
  if (clipIds.length === 0) {
    fail("empty-edit", "edit has no target");
  }
  commit(seq, (draft) => {
    const ids = expandLinks(draft, clipIds);
    const spans: Array<[string, number, number]> = [];
    for (const id of ids) {
      const trackId = trackIdOf(draft, id);
      const track = trackRef(draft, trackId);
      if (track.locked) {
        fail("track-locked", `track is locked: ${trackId}`);
      }
      const clip = clipRef(draft, id);
      spans.push([trackId, clip.timelineIn, clip.timelineOut]);
    }
    for (const id of ids) {
      const track = trackRef(draft, trackIdOf(draft, id));
      track.clips = track.clips.filter((clip) => clip.id !== id);
    }
    cleanupLinks(draft);
    spans.sort((a, b) => b[1] - a[1] || b[2] - a[2]);
    const seenOnTrack = new Set<string>();
    const synced = new Set<string>();
    const pairs: Array<[string, string]> = [];
    for (const [trackId, start, end] of spans) {
      const trackKey = `${trackId}:${start}:${end}`;
      if (seenOnTrack.has(trackKey)) {
        continue;
      }
      seenOnTrack.add(trackKey);
      shiftStartingAt(trackRef(draft, trackId), end, start - end, []);
      const syncKey = `${start}:${end}`;
      if (synced.has(syncKey)) {
        continue;
      }
      synced.add(syncKey);
      const deletedTracks = new Set(spans.filter((span) => span[1] === start && span[2] === end).map((span) => span[0]));
      for (const track of allTracks(draft)) {
        if (!track.syncLock || deletedTracks.has(track.id)) {
          continue;
        }
        if (track.locked) {
          fail("track-locked", `track is locked: ${track.id}`);
        }
        pairs.push(...removeTime(track, start, end).splitPairs);
      }
    }
    relinkSplitRights(draft, pairs);
  });
}

export function linkClips(seq: Sequence, clipIds: readonly string[]): void {
  if (clipIds.length < 2) {
    fail("empty-edit", "edit has no target");
  }
  commit(seq, (draft) => {
    const group: string[] = [];
    for (const id of clipIds) {
      for (const linked of expandLinks(draft, [id])) {
        if (!group.includes(linked)) {
          group.push(linked);
        }
      }
    }
    assignLinks(draft, group);
  });
}

export function unlinkClip(seq: Sequence, clipId: string): void {
  commit(seq, (draft) => {
    const clip = clipRef(draft, clipId);
    const partners = linksOf(clip);
    clip.linkedIds = [];
    for (const partner of partners) {
      const other = allTracks(draft)
        .flatMap((track) => track.clips)
        .find((item) => item.id === partner);
      if (other?.linkedIds) {
        other.linkedIds = other.linkedIds.filter((id) => id !== clipId);
      }
    }
  });
}

export function setSpeed(seq: Sequence, clipId: string, speed: number, ripple: boolean): void {
  if (!Number.isFinite(speed) || speed <= 0) {
    fail("invalid-speed", "speed must be a finite number greater than zero");
  }
  commit(seq, (draft) => {
    const changes: Array<[string, number, string, number, number | undefined]> = [];
    for (const id of expandLinks(draft, [clipId])) {
      const trackId = trackIdOf(draft, id);
      if (trackRef(draft, trackId).locked) {
        fail("track-locked", `track is locked: ${trackId}`);
      }
      const clip = clipRef(draft, id);
      const source = clip.sourceOut - clip.sourceIn;
      if (source <= 0) {
        fail("insufficient-source", "not enough source media for this edit");
      }
      const newDuration = Math.round(source / speed);
      if (newDuration <= 0) {
        fail("zero-duration", "clip duration must stay positive");
      }
      changes.push([trackId, clip.timelineOut, id, newDuration, speed]);
    }
    applyRetiming(draft, clipId, changes, ripple);
  });
}

export function setDuration(seq: Sequence, clipId: string, durationTicks: number, ripple: boolean): void {
  if (durationTicks <= 0) {
    fail("zero-duration", "clip duration must stay positive");
  }
  commit(seq, (draft) => {
    const changes: Array<[string, number, string, number, number | undefined]> = [];
    for (const id of expandLinks(draft, [clipId])) {
      const trackId = trackIdOf(draft, id);
      if (trackRef(draft, trackId).locked) {
        fail("track-locked", `track is locked: ${trackId}`);
      }
      const clip = clipRef(draft, id);
      const source = clip.sourceOut - clip.sourceIn;
      let newSpeed: number | undefined;
      if (source > 0) {
        newSpeed = source / durationTicks;
      } else if (clip.speed === 0) {
        newSpeed = undefined;
      } else {
        fail("insufficient-source", "not enough source media for this edit");
      }
      changes.push([trackId, clip.timelineOut, id, durationTicks, newSpeed]);
    }
    applyRetiming(draft, clipId, changes, ripple);
  });
}

function applyRetiming(
  seq: Sequence,
  primaryId: string,
  changes: Array<[string, number, string, number, number | undefined]>,
  ripple: boolean,
): void {
  const primary = changes.find((change) => change[2] === primaryId);
  if (!primary) {
    fail("clip-not-found", `clip not found: ${primaryId}`);
  }
  for (const [, , id, newDuration, newSpeed] of changes) {
    const clip = clipRef(seq, id);
    if (newSpeed !== undefined) {
      clip.speed = newSpeed;
    }
    clip.timelineOut = clip.timelineIn + newDuration;
  }
  if (!ripple) {
    return;
  }
  const ordered = [...changes].sort((a, b) => b[1] - a[1]);
  const seen = new Set<string>();
  const handled = new Set<string>();
  const pairs: Array<[string, string]> = [];
  for (const [trackId, oldOut, id, newDuration] of ordered) {
    handled.add(trackId);
    const clipIn = clipRef(seq, id).timelineIn;
    const delta = clipIn + newDuration - oldOut;
    const key = `${trackId}:${oldOut}`;
    if (delta === 0 || seen.has(key)) {
      continue;
    }
    seen.add(key);
    const ignore = ordered.filter((change) => change[0] === trackId).map((change) => change[2]);
    pairs.push(...rippleFrom(trackRef(seq, trackId), oldOut, delta, ignore));
  }
  const primaryDelta = clipRef(seq, primaryId).timelineIn + primary[3] - primary[1];
  if (primaryDelta !== 0) {
    for (const track of allTracks(seq)) {
      if (handled.has(track.id) || !track.syncLock) {
        continue;
      }
      if (track.locked) {
        fail("track-locked", `track is locked: ${track.id}`);
      }
      pairs.push(...rippleFrom(track, primary[1], primaryDelta, []));
    }
  }
  relinkSplitRights(seq, pairs);
}

export function setReversed(seq: Sequence, clipId: string, reversed: boolean): void {
  commit(seq, (draft) => {
    for (const id of expandLinks(draft, [clipId])) {
      clipRef(draft, id).reversed = reversed;
    }
  });
}

function markerList(seq: Sequence): Marker[] {
  if (!seq.markers) {
    seq.markers = [];
  }
  return seq.markers;
}

export function addMarker(seq: Sequence, draft: MarkerDraft): string {
  if (draft.timeTicks < 0 || draft.durationTicks < 0) {
    fail("invalid-time", "time is outside the sequence");
  }
  const marker: Marker = {
    id: newId("marker"),
    timeTicks: draft.timeTicks,
    durationTicks: draft.durationTicks,
    name: draft.name,
    comment: draft.comment,
    color: draft.color,
    kind: draft.kind,
  };
  const markers = markerList(seq);
  markers.push(marker);
  markers.sort((a, b) => a.timeTicks - b.timeTicks);
  return marker.id;
}

export function moveMarker(seq: Sequence, markerId: string, timeTicks: number): void {
  if (timeTicks < 0) {
    fail("invalid-time", "time is outside the sequence");
  }
  const markers = markerList(seq);
  const marker = markers.find((item) => item.id === markerId);
  if (!marker) {
    fail("marker-not-found", `marker not found: ${markerId}`);
  }
  marker.timeTicks = timeTicks;
  markers.sort((a, b) => a.timeTicks - b.timeTicks);
}

export function removeMarker(seq: Sequence, markerId: string): Marker {
  const markers = markerList(seq);
  const index = markers.findIndex((item) => item.id === markerId);
  if (index < 0) {
    fail("marker-not-found", `marker not found: ${markerId}`);
  }
  const [removed] = markers.splice(index, 1);
  if (!removed) {
    fail("marker-not-found", `marker not found: ${markerId}`);
  }
  return removed;
}

export function collectSnapPoints(
  seq: Sequence,
  playhead?: number,
  ignoreClipIds: readonly string[] = [],
): Array<[number, SnapKind]> {
  const points: Array<[number, SnapKind]> = [[0, "sequence-start"]];
  if (playhead !== undefined) {
    points.push([playhead, "playhead"]);
  }
  for (const track of allTracks(seq)) {
    for (const clip of track.clips) {
      if (ignoreClipIds.includes(clip.id)) {
        continue;
      }
      points.push([clip.timelineIn, "clip-in"], [clip.timelineOut, "clip-out"]);
    }
  }
  for (const marker of seq.markers ?? []) {
    points.push([marker.timeTicks, "marker"]);
  }
  return points;
}

export function snapHit(ticks: number, points: Array<[number, SnapKind]>, threshold: number): SnapHit | undefined {
  const limit = Math.max(0, threshold);
  let best: SnapHit | undefined;
  for (const [point, kind] of points) {
    const distance = Math.abs(point - ticks);
    if (distance > limit) {
      continue;
    }
    if (!best || distance < best.distance || (distance === best.distance && point < best.ticks)) {
      best = { ticks: point, kind, distance };
    }
  }
  return best;
}

export function snapTicks(
  seq: Sequence,
  ticks: number,
  threshold: number,
  playhead?: number,
  ignoreClipIds: readonly string[] = [],
): number {
  return snapHit(ticks, collectSnapPoints(seq, playhead, ignoreClipIds), threshold)?.ticks ?? ticks;
}

export function nest(seq: Sequence, clipIds: readonly string[], name: string): Sequence {
  return commit(seq, (draft) => {
    const ids = expandLinks(draft, clipIds);
    const moved: Array<[string, Clip]> = [];
    for (const id of ids) {
      const trackId = trackIdOf(draft, id);
      if (trackRef(draft, trackId).locked) {
        fail("track-locked", `track is locked: ${trackId}`);
      }
      moved.push([trackId, clone(clipRef(draft, id))]);
    }
    const minIn = Math.min(...moved.map(([, clip]) => clip.timelineIn));
    const maxOut = Math.max(...moved.map(([, clip]) => clip.timelineOut));
    if (!Number.isFinite(minIn) || !Number.isFinite(maxOut)) {
      fail("empty-edit", "edit has no target");
    }
    if (maxOut <= minIn) {
      fail("zero-duration", "clip duration must stay positive");
    }
    const destination = nestDestination(draft, moved);
    const child: Sequence = {
      id: newId("seq"),
      name,
      settings: clone(draft.settings),
      videoTracks: [],
      audioTracks: [],
      markers: [],
      captions: [],
      multicamAngles: [],
    };
    for (const parent of allTracks(draft)) {
      const clips = moved
        .filter(([trackId]) => trackId === parent.id)
        .map(([, clip]) => clip);
      if (clips.length === 0) {
        continue;
      }
      for (const clip of clips) {
        clip.timelineIn -= minIn;
        clip.timelineOut -= minIn;
      }
      clips.sort((a, b) => a.timelineIn - b.timelineIn);
      const track: Track = {
        id: newId("track"),
        index: parent.index,
        kind: parent.kind,
        clips,
        transitions: [],
        locked: false,
        muted: false,
        hidden: false,
        syncLock: parent.syncLock,
        targeted: parent.targeted,
      };
      pushTrack(child, track);
    }
    for (const id of ids) {
      const track = trackRef(draft, trackIdOf(draft, id));
      track.clips = track.clips.filter((clip) => clip.id !== id);
    }
    cleanupLinks(draft);
    const track = trackRef(draft, destination);
    if (track.clips.some((clip) => clip.timelineIn < maxOut && clip.timelineOut > minIn)) {
      fail("overlap", "edit would overlap another clip");
    }
    const nested: Clip = {
      id: newId("clip"),
      name,
      kind: "nested",
      nestedSequenceId: child.id,
      timelineIn: minIn,
      timelineOut: maxOut,
      sourceIn: 0,
      sourceOut: maxOut - minIn,
      speed: 1,
      reversed: false,
      enabled: true,
      labelColor: 0,
    };
    track.clips.push(nested);
    sortClips(track);
    return child;
  });
}

function pushTrack(seq: Sequence, track: Track): void {
  switch (track.kind) {
    case "video":
      seq.videoTracks.push(track);
      break;
    case "audio":
      seq.audioTracks.push(track);
      break;
    default: {
      const unexpected: never = track.kind;
      fail("invalid-time", `unexpected track kind ${String(unexpected)}`);
    }
  }
}

function nestDestination(seq: Sequence, moved: Array<[string, Clip]>): string {
  const ranked: Array<[number, number, string]> = [];
  for (const [trackId] of moved) {
    const track = trackRef(seq, trackId);
    const kindOrder = kindRank(track.kind);
    ranked.push([kindOrder, track.index, track.id]);
  }
  ranked.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const first = ranked[0];
  if (!first) {
    fail("empty-edit", "edit has no target");
  }
  return first[2];
}

function kindRank(kind: TrackKind): number {
  switch (kind) {
    case "video":
      return 0;
    case "audio":
      return 1;
    default: {
      const unexpected: never = kind;
      return unexpected;
    }
  }
}

export function addMulticamAngle(seq: Sequence, name: string, mediaId: string, syncTicks: number): string {
  const angle = {
    id: newId("angle"),
    name,
    mediaId,
    syncTicks,
  };
  if (!seq.multicamAngles) {
    seq.multicamAngles = [];
  }
  seq.multicamAngles.push(angle);
  return angle.id;
}

export function switchMulticamAngle(seq: Sequence, clipId: string, at: number, angleId: string): string {
  return commit(seq, (draft) => {
    const angle = (draft.multicamAngles ?? []).find((item) => item.id === angleId);
    if (!angle) {
      fail("angle-not-found", `multicam angle not found: ${angleId}`);
    }
    const original = clipRef(draft, clipId);
    if (at < original.timelineIn || at >= original.timelineOut) {
      fail("not-under-playhead", "playhead is not inside the clip");
    }
    const oldSync = (draft.multicamAngles ?? []).find((item) => original.mediaId === item.mediaId)?.syncTicks ?? 0;
    const deltaSync = angle.syncTicks - oldSync;
    const trackId = trackIdOf(draft, clipId);
    const target = at === original.timelineIn
      ? clipId
      : razorInner(draft, at, [trackId]).find(([left]) => left === clipId)?.[1];
    if (!target) {
      fail("not-under-playhead", "playhead is not inside the clip");
    }
    const clip = clipRef(draft, target);
    const sourceIn = clip.sourceIn + deltaSync;
    const sourceOut = clip.sourceOut + deltaSync;
    if (sourceIn < 0 || (clip.speed !== 0 && sourceOut <= sourceIn)) {
      fail("insufficient-source", "not enough source media for this edit");
    }
    clip.sourceIn = sourceIn;
    clip.sourceOut = sourceOut;
    clip.mediaId = angle.mediaId;
    return target;
  });
}

export function freezeFrame(seq: Sequence, clipId: string, at: number, holdTicks: number): string {
  if (holdTicks <= 0) {
    fail("zero-duration", "clip duration must stay positive");
  }
  return commit(seq, (draft) => {
    const trackId = trackIdOf(draft, clipId);
    const clip = clipRef(draft, clipId);
    if (at < clip.timelineIn || at >= clip.timelineOut) {
      fail("not-under-playhead", "playhead is not inside the clip");
    }
    const frame = sourceAt(clip, at);
    if (at > clip.timelineIn) {
      razorInner(draft, at, [trackId]);
    }
    const freeze: Clip = {
      id: newId("clip"),
      name: `${clip.name} Freeze`,
      mediaId: clip.mediaId,
      nestedSequenceId: clip.nestedSequenceId,
      kind: clip.kind,
      timelineIn: at,
      timelineOut: at + holdTicks,
      sourceIn: frame,
      sourceOut: frame,
      speed: 0,
      reversed: false,
      enabled: clip.enabled,
      labelColor: clip.labelColor,
    };
    insert(draft, at, [{ trackId, clip: freeze }]);
    return freeze.id;
  });
}
