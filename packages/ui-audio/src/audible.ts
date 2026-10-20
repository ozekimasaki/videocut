export interface AudibleTrack {
  id: string;
  muted: boolean;
  solo: boolean;
}

export function isAudible(muted: boolean, solo: boolean, anySolo: boolean): boolean {
  return !muted && (!anySolo || solo);
}

export function audibleTrackIds(tracks: readonly AudibleTrack[]): string[] {
  const anySolo = tracks.some((track) => track.solo);
  return tracks
    .filter((track) => isAudible(track.muted, track.solo, anySolo))
    .map((track) => track.id);
}
