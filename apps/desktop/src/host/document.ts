import type { Clip, EffectInstance, Project, Sequence, Track, Transition } from "@videocut/model";
import type { EditCommand } from "./command-stack";

export function projectCommand(label: string, apply: (project: Project) => Project): EditCommand {
  return { label, apply };
}

export function editSequence(
  project: Project,
  sequenceId: string,
  edit: (sequence: Sequence) => Sequence | null,
): Project {
  let changed = false;
  const sequences = project.sequences.map((sequence) => {
    if (sequence.id !== sequenceId) return sequence;
    const next = edit(sequence);
    if (!next) return sequence;
    changed = true;
    return next;
  });
  return changed ? { ...project, sequences } : project;
}

export function editClip(
  project: Project,
  sequenceId: string,
  clipId: string,
  edit: (clip: Clip) => Clip | null,
): Project {
  return editSequence(project, sequenceId, (sequence) => {
    let changed = false;
    const mapTrack = (track: Track): Track => ({
      ...track,
      clips: track.clips.map((clip) => {
        if (clip.id !== clipId) return clip;
        const next = edit(clip);
        if (!next) return clip;
        changed = true;
        return next;
      }),
    });
    const videoTracks = sequence.videoTracks.map(mapTrack);
    const audioTracks = sequence.audioTracks.map(mapTrack);
    if (!changed) return null;
    return { ...sequence, videoTracks, audioTracks };
  });
}

export function replaceClipEffects(
  project: Project,
  sequenceId: string,
  clipId: string,
  effects: EffectInstance[],
): Project {
  return editClip(project, sequenceId, clipId, (clip) => ({ ...clip, effects }));
}

export function replaceTransition(project: Project, sequenceId: string, transition: Transition): Project {
  return editSequence(project, sequenceId, (sequence) => {
    let changed = false;
    const mapTrack = (track: Track): Track => ({
      ...track,
      transitions: (track.transitions ?? []).map((item) => {
        if (item.id !== transition.id) return item;
        changed = true;
        return transition;
      }),
    });
    if (!changed) return null;
    return {
      ...sequence,
      videoTracks: sequence.videoTracks.map(mapTrack),
      audioTracks: sequence.audioTracks.map(mapTrack),
    };
  });
}

export function transitionNearClip(track: Track, clip: Clip): Transition | null {
  const transitions = track.transitions ?? [];
  return (
    transitions.find((item) => {
      const half = item.durationTicks / 2;
      const start = item.centerTicks - half;
      const end = item.centerTicks + half;
      return end > clip.timelineIn && start < clip.timelineOut;
    }) ?? null
  );
}
