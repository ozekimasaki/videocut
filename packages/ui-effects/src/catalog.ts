import { effectIds, type EffectInstance, type ParamValue } from "@videocut/model";

import { cloneValue, type DisplayScale } from "./values";

export interface ParamSpec {
  id: string;
  label: string;
  display: DisplayScale;
  min?: number;
  max?: number;
  defaultValue: ParamValue;
}

export interface EffectSpec {
  effectId: string;
  label: string;
  kind: "filter" | "transition";
  params: ParamSpec[];
}

const point = (x: number, y: number): ParamValue => ({ type: "point", value: { x, y } });
const float = (value: number): ParamValue => ({ type: "float", value });

const placement: ParamSpec[] = [
  {
    id: "anchor",
    label: "Anchor",
    display: "number",
    min: 0,
    max: 1,
    defaultValue: point(0.5, 0.5),
  },
  {
    id: "position",
    label: "Position",
    display: "number",
    defaultValue: point(0.5, 0.5),
  },
  {
    id: "scale",
    label: "Scale",
    display: "percent",
    defaultValue: point(1, 1),
  },
  {
    id: "rotation",
    label: "Rotation",
    display: "degree",
    defaultValue: float(0),
  },
];

const catalog: EffectSpec[] = [
  { effectId: effectIds.motion, label: "Motion", kind: "filter", params: placement },
  {
    effectId: effectIds.opacity,
    label: "Opacity",
    kind: "filter",
    params: [
      {
        id: "opacity",
        label: "Opacity",
        display: "percent",
        min: 0,
        max: 1,
        defaultValue: float(1),
      },
    ],
  },
  {
    effectId: effectIds.crop,
    label: "Crop",
    kind: "filter",
    params: ["left", "top", "right", "bottom"].map((id) => ({
      id,
      label: id.charAt(0).toUpperCase() + id.slice(1),
      display: "percent" as const,
      min: 0,
      max: 1,
      defaultValue: float(0),
    })),
  },
  {
    effectId: effectIds.transform,
    label: "Transform",
    kind: "filter",
    params: [
      ...placement,
      { id: "skew", label: "Skew", display: "degree", defaultValue: float(0) },
      { id: "skewAxis", label: "Skew Axis", display: "degree", defaultValue: float(0) },
    ],
  },
  { effectId: effectIds.crossDissolve, label: "Cross Dissolve", kind: "transition", params: [] },
  { effectId: effectIds.dipToBlack, label: "Dip to Black", kind: "transition", params: [] },
  { effectId: effectIds.dipToWhite, label: "Dip to White", kind: "transition", params: [] },
  { effectId: effectIds.wipe, label: "Wipe", kind: "transition", params: [] },
];

export function effectSpecs(): readonly EffectSpec[] {
  return catalog;
}

export function effectSpec(effectId: string): EffectSpec | undefined {
  return catalog.find((spec) => spec.effectId === effectId);
}

export function effectLabel(effectId: string): string {
  return effectSpec(effectId)?.label ?? effectId;
}

export function paramSpec(effectId: string, paramId: string): ParamSpec | undefined {
  return effectSpec(effectId)?.params.find((param) => param.id === paramId);
}

export function createEffect(effectId: string): EffectInstance | undefined {
  const spec = effectSpec(effectId);
  if (!spec || spec.kind !== "filter") return undefined;
  return {
    id: newId("fx"),
    effectId,
    enabled: true,
    params: spec.params.map((param) => ({
      id: param.id,
      value: cloneValue(param.defaultValue),
      keyframes: [],
    })),
  };
}

export function defaultVideoStack(): EffectInstance[] {
  const motion = createEffect(effectIds.motion);
  const opacity = createEffect(effectIds.opacity);
  if (!motion || !opacity) return [];
  return [motion, opacity];
}

function newId(prefix: string): string {
  const uuid = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  return `${prefix}-${uuid}`;
}
