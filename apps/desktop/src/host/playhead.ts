import type { Clip, ClipKind, Project, Sequence, Track } from "@videocut/model";

export interface ClipHit {
  track: Track;
  clip: Clip;
}

export function sequenceById(project: Project, sequenceId: string | null): Sequence | null {
  if (!sequenceId) return null;
  return project.sequences.find((sequence) => sequence.id === sequenceId) ?? null;
}

export function topClipAt(sequence: Sequence, ticks: number): ClipHit | null {
  const tracks = [...sequence.videoTracks].sort((left, right) => right.index - left.index);
  for (const track of tracks) {
    if (track.hidden) continue;
    const clip = track.clips.find((item) => item.enabled && item.timelineIn <= ticks && ticks < item.timelineOut);
    if (clip) return { track, clip };
  }
  return null;
}

export function sourceTicksAt(clip: Clip, timelineTicks: number): number {
  const offset = Math.max(0, timelineTicks - clip.timelineIn) * clip.speed;
  if (clip.reversed) return Math.round(clip.sourceOut - offset);
  return Math.round(clip.sourceIn + offset);
}

export function sequenceEnd(sequence: Sequence): number {
  const ends = [...sequence.videoTracks, ...sequence.audioTracks].flatMap((track) => track.clips.map((clip) => clip.timelineOut));
  return ends.reduce((max, value) => Math.max(max, value), 0);
}

export function findClip(sequence: Sequence, clipId: string): ClipHit | null {
  for (const track of [...sequence.videoTracks, ...sequence.audioTracks]) {
    const clip = track.clips.find((item) => item.id === clipId);
    if (clip) return { track, clip };
  }
  return null;
}

export interface MonitorPicture {
  title: string;
  timecodeTicks: number;
  durationTicks: number;
  clipName: string | null;
  kind: ClipKind | null;
  labelColor: number;
  progress: number;
  mediaPath: string | null;
  frameTicks: number;
  width: number;
  height: number;
  overlayText: string | null;
}

export function programPicture(project: Project, sequence: Sequence | null, playheadTicks: number): MonitorPicture {
  const empty = blank("プログラム", playheadTicks, sequence);
  if (!sequence) return empty;
  const overlayText = captionAt(sequence, playheadTicks);
  const hit = topClipAt(sequence, playheadTicks);
  if (!hit) return { ...empty, overlayText, durationTicks: Math.max(sequenceEnd(sequence), playheadTicks) };
  const span = Math.max(1, hit.clip.timelineOut - hit.clip.timelineIn);
  const media = hit.clip.mediaId ? project.media.find((item) => item.id === hit.clip.mediaId) : undefined;
  return {
    title: "プログラム",
    timecodeTicks: playheadTicks,
    durationTicks: Math.max(sequenceEnd(sequence), playheadTicks),
    clipName: hit.clip.name,
    kind: hit.clip.kind,
    labelColor: hit.clip.labelColor,
    progress: (playheadTicks - hit.clip.timelineIn) / span,
    mediaPath: media?.path ?? null,
    frameTicks: sourceTicksAt(hit.clip, playheadTicks),
    width: sequence.settings.width,
    height: sequence.settings.height,
    overlayText,
  };
}

export function sourcePicture(
  project: Project,
  sequence: Sequence | null,
  sourceMediaId: string | null,
  sourceClipId: string | null,
  sourcePlayheadTicks: number,
): MonitorPicture {
  const media = sourceMediaId ? project.media.find((item) => item.id === sourceMediaId) : undefined;
  if (media) {
    const duration = media.durationTicks > 0 ? media.durationTicks : 0;
    const playhead = Math.max(0, sourcePlayheadTicks);
    return {
      title: "ソース",
      timecodeTicks: playhead,
      durationTicks: duration,
      clipName: media.name,
      kind: "media",
      labelColor: 6,
      progress: duration > 0 ? Math.min(1, playhead / duration) : 0,
      mediaPath: media.path,
      frameTicks: playhead,
      width: media.video?.width ?? sequence?.settings.width ?? 1920,
      height: media.video?.height ?? sequence?.settings.height ?? 1080,
      overlayText: null,
    };
  }
  if (sequence && sourceClipId) {
    const hit = findClip(sequence, sourceClipId);
    if (hit) {
      const duration = Math.max(0, hit.clip.sourceOut - hit.clip.sourceIn);
      const playhead = Math.min(duration, Math.max(0, sourcePlayheadTicks));
      return {
        title: "ソース",
        timecodeTicks: playhead,
        durationTicks: duration,
        clipName: hit.clip.name,
        kind: hit.clip.kind,
        labelColor: hit.clip.labelColor,
        progress: duration > 0 ? playhead / duration : 0,
        mediaPath: null,
        frameTicks: hit.clip.sourceIn + playhead,
        width: sequence.settings.width,
        height: sequence.settings.height,
        overlayText: null,
      };
    }
  }
  return blank("ソース", sourcePlayheadTicks, sequence);
}

function captionAt(sequence: Sequence, ticks: number): string | null {
  const caption = (sequence.captions ?? []).find((item) => item.startTicks <= ticks && ticks < item.endTicks);
  return caption?.text ?? null;
}

function blank(title: string, ticks: number, sequence: Sequence | null): MonitorPicture {
  return {
    title,
    timecodeTicks: ticks,
    durationTicks: 0,
    clipName: null,
    kind: null,
    labelColor: 0,
    progress: 0,
    mediaPath: null,
    frameTicks: 0,
    width: sequence?.settings.width ?? 1920,
    height: sequence?.settings.height ?? 1080,
    overlayText: null,
  };
}

export function snapTicks(ticks: number, points: readonly number[], threshold: number): number {
  let best = ticks;
  let bestDistance = threshold;
  for (const point of points) {
    const distance = Math.abs(point - ticks);
    if (distance <= bestDistance) {
      best = point;
      bestDistance = distance;
    }
  }
  return best;
}

export function clipsStartingAt(sequence: Sequence, ticks: number): string[] {
  return [...sequence.videoTracks, ...sequence.audioTracks].flatMap((track) =>
    track.clips.filter((clip) => clip.timelineIn === ticks).map((clip) => clip.id),
  );
}
