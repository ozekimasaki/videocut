/** Mirrors `crates/model`. Rust is the source of truth. Field names are camelCase. */

export const TICKS_PER_SECOND = 254_016_000_000;

export interface Rational {
  num: number;
  den: number;
}

export type BinItemKind = "bin" | "clip" | "sequence";
export type TrackKind = "video" | "audio";
export type ClipKind =
  | "media"
  | "nested"
  | "adjustment"
  | "title"
  | "colorMatte"
  | "caption";
export type Interp = "linear" | "bezier" | "hold";
export type TransitionAlign = "start" | "center" | "end";
export type MarkerKind = "comment" | "chapter" | "segmentation" | "caption";

export interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

export interface Point2 {
  x: number;
  y: number;
}

export type ParamValue =
  | { type: "float"; value: number }
  | { type: "int"; value: number }
  | { type: "bool"; value: boolean }
  | { type: "color"; value: Rgba }
  | { type: "point"; value: Point2 }
  | { type: "str"; value: string };

export interface Keyframe {
  timeTicks: number;
  value: ParamValue;
  interp: Interp;
}

export interface Param {
  id: string;
  value: ParamValue;
  keyframes?: Keyframe[];
}

export interface EffectInstance {
  id: string;
  effectId: string;
  enabled: boolean;
  params: Param[];
}

export interface Transition {
  id: string;
  effectId: string;
  centerTicks: number;
  durationTicks: number;
  alignment: TransitionAlign;
}

export interface Clip {
  id: string;
  name: string;
  mediaId?: string;
  nestedSequenceId?: string;
  kind: ClipKind;
  timelineIn: number;
  timelineOut: number;
  sourceIn: number;
  sourceOut: number;
  speed: number;
  reversed: boolean;
  enabled: boolean;
  linkedIds?: string[];
  effects?: EffectInstance[];
  labelColor: number;
}

export interface Track {
  id: string;
  index: number;
  kind: TrackKind;
  clips: Clip[];
  transitions?: Transition[];
  locked: boolean;
  muted: boolean;
  hidden: boolean;
  syncLock: boolean;
  targeted: boolean;
}

export interface Marker {
  id: string;
  timeTicks: number;
  durationTicks: number;
  name: string;
  comment: string;
  color: number;
  kind: MarkerKind;
}

export interface Caption {
  id: string;
  startTicks: number;
  endTicks: number;
  text: string;
}

export interface MulticamAngle {
  id: string;
  name: string;
  mediaId: string;
  syncTicks: number;
}

export interface SequenceSettings {
  width: number;
  height: number;
  fps: Rational;
  sampleRate: number;
  audioChannels: number;
  startTimecodeTicks: number;
}

export interface Sequence {
  id: string;
  name: string;
  settings: SequenceSettings;
  videoTracks: Track[];
  audioTracks: Track[];
  markers?: Marker[];
  captions?: Caption[];
  multicamAngles?: MulticamAngle[];
}

export interface VideoStreamInfo {
  width: number;
  height: number;
  fps: Rational;
  codec: string;
}

export interface AudioStreamInfo {
  sampleRate: number;
  channels: number;
  codec: string;
}

export interface MediaRef {
  id: string;
  name: string;
  path: string;
  durationTicks: number;
  video?: VideoStreamInfo;
  audio?: AudioStreamInfo;
  proxyPath?: string;
}

export interface BinNode {
  id: string;
  name: string;
  kind: BinItemKind;
  mediaId?: string;
  sequenceId?: string;
  children?: BinNode[];
}

export interface Project {
  id: string;
  name: string;
  bins: BinNode[];
  sequences: Sequence[];
  media: MediaRef[];
}

export const effectIds = {
  motion: "builtin.motion",
  opacity: "builtin.opacity",
  crop: "builtin.crop",
  transform: "builtin.transform",
  crossDissolve: "builtin.cross-dissolve",
  dipToBlack: "builtin.dip-to-black",
  dipToWhite: "builtin.dip-to-white",
  wipe: "builtin.wipe",
  gain: "builtin.gain",
  pan: "builtin.pan",
  eq: "builtin.eq",
  compressor: "builtin.compressor",
  colorWheels: "builtin.color-wheels",
  curves: "builtin.curves",
  hsl: "builtin.hsl",
  lut: "builtin.lut",
} as const;
