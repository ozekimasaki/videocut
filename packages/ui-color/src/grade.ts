import { effectIds, type EffectInstance, type Param, type ParamValue } from "@videocut/model";
import { CURVE_IDENTITY } from "./curves";

export { CURVE_IDENTITY };

/** Parameter ids shared with `videocut_color`. */
export const colorParams = {
  shadowsX: "shadowsX",
  shadowsY: "shadowsY",
  shadowsMaster: "shadowsMaster",
  midtonesX: "midtonesX",
  midtonesY: "midtonesY",
  midtonesMaster: "midtonesMaster",
  highlightsX: "highlightsX",
  highlightsY: "highlightsY",
  highlightsMaster: "highlightsMaster",
  master: "master",
  red: "red",
  green: "green",
  blue: "blue",
  hue: "hue",
  saturation: "saturation",
  lightness: "lightness",
  hueCenter: "hueCenter",
  hueWidth: "hueWidth",
  satLow: "satLow",
  satHigh: "satHigh",
  lumLow: "lumLow",
  lumHigh: "lumHigh",
  softness: "softness",
  cube: "cube",
  intensity: "intensity",
} as const;

export type ColorTool = "wheels" | "curves" | "hsl" | "lut";

const COLOR_IDS: readonly string[] = [
  effectIds.colorWheels,
  effectIds.curves,
  effectIds.hsl,
  effectIds.lut,
];

export function isColorEffect(effectId: string): boolean {
  return COLOR_IDS.includes(effectId);
}

export function toolForEffect(effectId: string): ColorTool | null {
  switch (effectId) {
    case effectIds.colorWheels:
      return "wheels";
    case effectIds.curves:
      return "curves";
    case effectIds.hsl:
      return "hsl";
    case effectIds.lut:
      return "lut";
    default:
      return null;
  }
}

export function colorToolLabel(tool: ColorTool): string {
  switch (tool) {
    case "wheels":
      return "Wheels";
    case "curves":
      return "Curves";
    case "hsl":
      return "HSL";
    case "lut":
      return "LUT";
    default: {
      const exhaustive: never = tool;
      return exhaustive;
    }
  }
}

export function readFloat(effect: EffectInstance, id: string, fallback: number): number {
  const param = effect.params.find((item) => item.id === id);
  if (!param) {
    return fallback;
  }
  if (param.value.type === "float" || param.value.type === "int") {
    return param.value.value;
  }
  return fallback;
}

export function readString(effect: EffectInstance, id: string, fallback: string): string {
  const param = effect.params.find((item) => item.id === id);
  if (!param || param.value.type !== "str") {
    return fallback;
  }
  return param.value.value;
}

export function setFloatParam(effect: EffectInstance, id: string, value: number): EffectInstance {
  return setParam(effect, id, { type: "float", value });
}

export function setStringParam(effect: EffectInstance, id: string, value: string): EffectInstance {
  return setParam(effect, id, { type: "str", value });
}

export function createColorWheelsEffect(): EffectInstance {
  return makeEffect(effectIds.colorWheels, [
    floatParam(colorParams.shadowsX, 0),
    floatParam(colorParams.shadowsY, 0),
    floatParam(colorParams.shadowsMaster, 0),
    floatParam(colorParams.midtonesX, 0),
    floatParam(colorParams.midtonesY, 0),
    floatParam(colorParams.midtonesMaster, 0),
    floatParam(colorParams.highlightsX, 0),
    floatParam(colorParams.highlightsY, 0),
    floatParam(colorParams.highlightsMaster, 0),
  ]);
}

export function createCurvesEffect(): EffectInstance {
  return makeEffect(effectIds.curves, [
    stringParam(colorParams.master, CURVE_IDENTITY),
    stringParam(colorParams.red, CURVE_IDENTITY),
    stringParam(colorParams.green, CURVE_IDENTITY),
    stringParam(colorParams.blue, CURVE_IDENTITY),
  ]);
}

export function createHslEffect(): EffectInstance {
  return makeEffect(effectIds.hsl, [
    floatParam(colorParams.hue, 0),
    floatParam(colorParams.saturation, 1),
    floatParam(colorParams.lightness, 0),
    floatParam(colorParams.hueCenter, 0),
    floatParam(colorParams.hueWidth, 360),
    floatParam(colorParams.satLow, 0),
    floatParam(colorParams.satHigh, 1),
    floatParam(colorParams.lumLow, 0),
    floatParam(colorParams.lumHigh, 1),
    floatParam(colorParams.softness, 0),
  ]);
}

export function createLutEffect(cube = ""): EffectInstance {
  return makeEffect(effectIds.lut, [
    stringParam(colorParams.cube, cube),
    floatParam(colorParams.intensity, 1),
  ]);
}

function makeEffect(effectId: string, params: Param[]): EffectInstance {
  return {
    id: `fx-${crypto.randomUUID()}`,
    effectId,
    enabled: true,
    params,
  };
}

function floatParam(id: string, value: number): Param {
  return { id, value: { type: "float", value } };
}

function stringParam(id: string, value: string): Param {
  return { id, value: { type: "str", value } };
}

function setParam(effect: EffectInstance, id: string, value: ParamValue): EffectInstance {
  const exists = effect.params.some((param) => param.id === id);
  const params = exists
    ? effect.params.map((param) => (param.id === id ? { ...param, value } : param))
    : [...effect.params, { id, value }];
  return { ...effect, params };
}
