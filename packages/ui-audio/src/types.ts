export interface MeterReading {
  peakDbfs: number;
  rmsDbfs?: number;
  clipped?: boolean;
}

export interface TrackStripState {
  trackId: string;
  solo: boolean;
  faderDb: number;
  pan: number;
}

export interface MixerState {
  masterGainDb: number;
  strips: TrackStripState[];
}

export interface MixerStripView {
  trackId: string;
  name: string;
  muted: boolean;
  solo: boolean;
  faderDb: number;
  pan: number;
  meters: MeterReading[];
  audible: boolean;
}
