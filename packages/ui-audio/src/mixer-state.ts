import type { Sequence } from "@videocut/model";

import { audibleTrackIds } from "./audible.ts";
import type { MeterReading, MixerState, MixerStripView } from "./types";

export function createMixerState(trackIds: readonly string[]): MixerState {
  return {
    masterGainDb: 0,
    strips: trackIds.map((trackId) => ({
      trackId,
      solo: false,
      faderDb: 0,
      pan: 0,
    })),
  };
}

export function mixerStateFromSequence(sequence: Sequence): MixerState {
  return createMixerState(sequence.audioTracks.map((track) => track.id));
}

export function stripViews(
  sequence: Sequence,
  mixer: MixerState,
  meters: Readonly<Record<string, readonly MeterReading[] | undefined>>,
): MixerStripView[] {
  const joined = sequence.audioTracks.map((track) => {
    const strip = mixer.strips.find((item) => item.trackId === track.id);
    return {
      id: track.id,
      index: track.index,
      muted: track.muted,
      solo: strip?.solo ?? false,
      faderDb: strip?.faderDb ?? 0,
      pan: strip?.pan ?? 0,
    };
  });
  const audible = new Set(
    audibleTrackIds(
      joined.map((track) => ({
        id: track.id,
        muted: track.muted,
        solo: track.solo,
      })),
    ),
  );
  return joined.map((track) => ({
    trackId: track.id,
    name: `A${track.index + 1}`,
    muted: track.muted,
    solo: track.solo,
    faderDb: track.faderDb,
    pan: track.pan,
    meters: [...(meters[track.id] ?? [])],
    audible: audible.has(track.id),
  }));
}
