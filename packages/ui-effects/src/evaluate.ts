import {
  effectIds,
  type EffectInstance,
  type Interp,
  type Keyframe,
  type Param,
  type ParamValue,
  type Point2,
  type Rgba,
  type Transition,
  type TransitionAlign,
} from "@videocut/model";

import { effectSpec } from "./catalog";
import { cloneValue } from "./values";

export interface Affine {
  a: number;
  b: number;
  c: number;
  d: number;
  tx: number;
  ty: number;
}

export interface Crop {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface EvaluatedParam {
  id: string;
  value: ParamValue;
}

export interface EvaluatedEffect {
  id: string;
  effectId: string;
  enabled: boolean;
  params: EvaluatedParam[];
}

export interface StackEvaluation {
  effects: EvaluatedEffect[];
  affine: Affine;
  opacity: number;
  crop: Crop;
}

export interface BlendWeights {
  outgoing: number;
  incoming: number;
  dip: number;
  dipColor: "none" | "black" | "white";
  wipeEdge: number;
  recognized: boolean;
}

/** Cubic ease with flat tangents. Matches `videocut_effects::bezier_ease`. */
export function bezierEase(t: number): number {
  const clamped = Math.min(1, Math.max(0, t));
  return clamped * clamped * (3 - 2 * clamped);
}

export function localTicks(timelineIn: number, timelineTicks: number): number {
  return timelineTicks - timelineIn;
}

export function evaluateParam(param: Param, localTick: number): ParamValue {
  const keys = param.keyframes ?? [];
  if (keys.length === 0) return cloneValue(param.value);
  const collapsed = collapseKeys(keys);
  const first = collapsed[0];
  const last = collapsed[collapsed.length - 1];
  if (!first || !last) return cloneValue(param.value);
  if (localTick <= first.timeTicks) return cloneValue(first.value);
  if (localTick >= last.timeTicks) return cloneValue(last.value);
  const index = collapsed.findIndex((key, position) => {
    const next = collapsed[position + 1];
    return next !== undefined && key.timeTicks <= localTick && localTick < next.timeTicks;
  });
  const left = collapsed[index];
  const right = collapsed[index + 1];
  if (!left || !right) return cloneValue(last.value);
  return interpolate(left, right, localTick);
}

export function evaluateStack(effects: readonly EffectInstance[], localTick: number): StackEvaluation {
  let affine = identity();
  let opacity = 1;
  let crop = emptyCrop();
  const evaluated: EvaluatedEffect[] = [];

  for (const effect of effects) {
    const params = evaluateEffectParams(effect, localTick);
    if (effect.enabled) {
      if (effect.effectId === effectIds.motion || effect.effectId === effectIds.transform) {
        affine = mul(placementMatrix(params, effect.effectId), affine);
      } else if (effect.effectId === effectIds.opacity) {
        opacity *= clamp01(paramFloat(params, "opacity", 1));
      } else if (effect.effectId === effectIds.crop) {
        crop = intersectCrop(crop, cropFrom(params));
      }
    }
    evaluated.push({
      id: effect.id,
      effectId: effect.effectId,
      enabled: effect.enabled,
      params,
    });
  }

  return { effects: evaluated, affine, opacity, crop };
}

export function applyAffine(affine: Affine, x: number, y: number): { x: number; y: number } {
  return {
    x: affine.a * x + affine.c * y + affine.tx,
    y: affine.b * x + affine.d * y + affine.ty,
  };
}

export function transitionBounds(transition: Transition): { start: number; end: number } {
  const duration = Math.max(0, transition.durationTicks);
  const cut = transition.centerTicks;
  switch (transition.alignment) {
    case "center": {
      const start = cut - Math.trunc(duration / 2);
      return { start, end: start + duration };
    }
    case "start":
      return { start: cut, end: cut + duration };
    case "end":
      return { start: cut - duration, end: cut };
    default: {
      const never: never = transition.alignment;
      return never;
    }
  }
}

export function transitionProgress(transition: Transition, tick: number): number | null {
  const { start, end } = transitionBounds(transition);
  if (tick < start || tick >= end) return null;
  const duration = Math.max(0, transition.durationTicks);
  if (duration === 0) return null;
  return (tick - start) / duration;
}

export function blendWeights(effectId: string, progress: number): BlendWeights {
  const clamped = clamp01(progress);
  switch (effectId) {
    case effectIds.crossDissolve:
      return mix(clamped, true);
    case effectIds.dipToBlack:
      return dip(clamped, "black");
    case effectIds.dipToWhite:
      return dip(clamped, "white");
    case effectIds.wipe:
      return {
        outgoing: 1 - clamped,
        incoming: clamped,
        dip: 0,
        dipColor: "none",
        wipeEdge: clamped,
        recognized: true,
      };
    default:
      return mix(clamped, false);
  }
}

export function isTransitionAlign(value: string): value is TransitionAlign {
  return value === "start" || value === "center" || value === "end";
}

export function isInterp(value: string): value is Interp {
  return value === "linear" || value === "hold" || value === "bezier";
}

function collapseKeys(keys: readonly Keyframe[]): Keyframe[] {
  const ordered = [...keys].sort((a, b) => a.timeTicks - b.timeTicks);
  const collapsed: Keyframe[] = [];
  for (const key of ordered) {
    const last = collapsed[collapsed.length - 1];
    if (last && last.timeTicks === key.timeTicks) collapsed.pop();
    collapsed.push(key);
  }
  return collapsed;
}

function interpolate(left: Keyframe, right: Keyframe, tick: number): ParamValue {
  const span = right.timeTicks - left.timeTicks;
  const progress = span === 0 ? 1 : clamp01((tick - left.timeTicks) / span);
  const eased = ease(left.interp, progress);
  const start = left.value;
  const end = right.value;
  if (start.type === "float" && end.type === "float") {
    return { type: "float", value: lerp(start.value, end.value, eased) };
  }
  if (start.type === "int" && end.type === "int") {
    return { type: "int", value: Math.round(lerp(start.value, end.value, eased)) };
  }
  if (start.type === "bool" && end.type === "bool") {
    return { type: "bool", value: eased >= 0.5 ? end.value : start.value };
  }
  if (start.type === "color" && end.type === "color") {
    return { type: "color", value: lerpRgba(start.value, end.value, eased) };
  }
  if (start.type === "point" && end.type === "point") {
    return { type: "point", value: lerpPoint(start.value, end.value, eased) };
  }
  return cloneValue(start);
}

function ease(interp: Interp, progress: number): number {
  switch (interp) {
    case "linear":
      return progress;
    case "hold":
      return 0;
    case "bezier":
      return bezierEase(progress);
    default: {
      const never: never = interp;
      return never;
    }
  }
}

function lerp(start: number, end: number, t: number): number {
  return start + (end - start) * t;
}

function lerpPoint(start: Point2, end: Point2, t: number): Point2 {
  return { x: lerp(start.x, end.x, t), y: lerp(start.y, end.y, t) };
}

function lerpRgba(start: Rgba, end: Rgba, t: number): Rgba {
  const channel = (from: number, to: number) => lerp(from, to, t);
  return {
    r: channel(start.r, end.r),
    g: channel(start.g, end.g),
    b: channel(start.b, end.b),
    a: channel(start.a, end.a),
  };
}

function evaluateEffectParams(effect: EffectInstance, localTick: number): EvaluatedParam[] {
  const spec = effectSpec(effect.effectId);
  const used = new Set<string>();
  const out: EvaluatedParam[] = [];
  if (spec) {
    for (const paramSpec of spec.params) {
      used.add(paramSpec.id);
      const param = effect.params.find((item) => item.id === paramSpec.id);
      out.push({
        id: paramSpec.id,
        value: param ? evaluateParam(param, localTick) : cloneValue(paramSpec.defaultValue),
      });
    }
  }
  for (const param of effect.params) {
    if (!used.has(param.id)) {
      out.push({ id: param.id, value: evaluateParam(param, localTick) });
    }
  }
  return out;
}

function placementMatrix(params: readonly EvaluatedParam[], effectId: string): Affine {
  const anchor = paramPoint(params, "anchor", { x: 0.5, y: 0.5 });
  const position = paramPoint(params, "position", { x: 0.5, y: 0.5 });
  const scale = paramPoint(params, "scale", { x: 1, y: 1 });
  const rotation = paramFloat(params, "rotation", 0);
  const skew = effectId === effectIds.transform ? paramFloat(params, "skew", 0) : 0;
  const skewAxis = effectId === effectIds.transform ? paramFloat(params, "skewAxis", 0) : 0;
  const skewMatrix = mul(mul(rotate(skewAxis), skewX(skew)), rotate(-skewAxis));
  return mul(
    mul(mul(mul(translate(position.x, position.y), rotate(rotation)), skewMatrix), scaleBy(scale.x, scale.y)),
    translate(-anchor.x, -anchor.y),
  );
}

function identity(): Affine {
  return { a: 1, b: 0, c: 0, d: 1, tx: 0, ty: 0 };
}

function translate(x: number, y: number): Affine {
  return { a: 1, b: 0, c: 0, d: 1, tx: x, ty: y };
}

function scaleBy(x: number, y: number): Affine {
  return { a: x, b: 0, c: 0, d: y, tx: 0, ty: 0 };
}

function rotate(degrees: number): Affine {
  const radians = (degrees * Math.PI) / 180;
  const sin = Math.sin(radians);
  const cos = Math.cos(radians);
  return { a: cos, b: sin, c: -sin, d: cos, tx: 0, ty: 0 };
}

function skewX(degrees: number): Affine {
  return { a: 1, b: 0, c: Math.tan((degrees * Math.PI) / 180), d: 1, tx: 0, ty: 0 };
}

function mul(self: Affine, other: Affine): Affine {
  return {
    a: self.a * other.a + self.c * other.b,
    b: self.b * other.a + self.d * other.b,
    c: self.a * other.c + self.c * other.d,
    d: self.b * other.c + self.d * other.d,
    tx: self.a * other.tx + self.c * other.ty + self.tx,
    ty: self.b * other.tx + self.d * other.ty + self.ty,
  };
}

function emptyCrop(): Crop {
  return { left: 0, top: 0, right: 0, bottom: 0 };
}

function intersectCrop(a: Crop, b: Crop): Crop {
  return {
    left: Math.max(a.left, b.left),
    top: Math.max(a.top, b.top),
    right: Math.max(a.right, b.right),
    bottom: Math.max(a.bottom, b.bottom),
  };
}

function cropFrom(params: readonly EvaluatedParam[]): Crop {
  return {
    left: clamp01(paramFloat(params, "left", 0)),
    top: clamp01(paramFloat(params, "top", 0)),
    right: clamp01(paramFloat(params, "right", 0)),
    bottom: clamp01(paramFloat(params, "bottom", 0)),
  };
}

function paramFloat(params: readonly EvaluatedParam[], id: string, fallback: number): number {
  const param = params.find((item) => item.id === id);
  if (!param) return fallback;
  if (param.value.type === "float") return param.value.value;
  if (param.value.type === "int") return param.value.value;
  return fallback;
}

function paramPoint(params: readonly EvaluatedParam[], id: string, fallback: Point2): Point2 {
  const param = params.find((item) => item.id === id);
  if (param?.value.type === "point") return param.value.value;
  return fallback;
}

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

function mix(progress: number, recognized: boolean): BlendWeights {
  return {
    outgoing: 1 - progress,
    incoming: progress,
    dip: 0,
    dipColor: "none",
    wipeEdge: 0,
    recognized,
  };
}

function dip(progress: number, dipColor: "black" | "white"): BlendWeights {
  const outgoing = Math.max(0, 1 - 2 * progress);
  const incoming = Math.max(0, 2 * progress - 1);
  return {
    outgoing,
    incoming,
    dip: clamp01(1 - outgoing - incoming),
    dipColor,
    wipeEdge: 0,
    recognized: true,
  };
}
