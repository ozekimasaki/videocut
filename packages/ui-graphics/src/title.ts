import type { Clip, Param, ParamValue } from "@videocut/model";
import { LOWER_THIRD_EFFECT_ID, TITLE_EFFECT_ID, paramIds } from "./schema.ts";

export type GraphicRole = "title" | "lowerThird";

export type EditResult<T> = { ok: true; value: T } | { ok: false; error: string };

export function graphicRole(clip: Clip): GraphicRole | null {
  const effects = clip.effects ?? [];
  if (effects.some((effect) => effect.effectId === LOWER_THIRD_EFFECT_ID)) {
    return "lowerThird";
  }
  if (effects.some((effect) => effect.effectId === TITLE_EFFECT_ID)) {
    return "title";
  }
  return null;
}

export function readTitleText(clip: Clip): string | null {
  return readStringParam(clip, TITLE_EFFECT_ID, paramIds.text);
}

export function readLowerThird(clip: Clip): { primary: string; secondary: string } | null {
  const primary = readStringParam(clip, LOWER_THIRD_EFFECT_ID, paramIds.primary);
  const secondary = readStringParam(clip, LOWER_THIRD_EFFECT_ID, paramIds.secondary);
  if (primary === null || secondary === null) {
    return null;
  }
  return { primary, secondary };
}

export function withTitleText(clip: Clip, text: string): EditResult<Clip> {
  if (graphicRole(clip) !== "title") {
    return { ok: false, error: "タイトルクリップではありません" };
  }
  const next = replaceParam(clip, TITLE_EFFECT_ID, paramIds.text, { type: "str", value: text });
  if (!next) {
    return { ok: false, error: "タイトルクリップではありません" };
  }
  return { ok: true, value: { ...next, name: text } };
}

export function withLowerThirdText(
  clip: Clip,
  primary: string,
  secondary: string,
): EditResult<Clip> {
  if (graphicRole(clip) !== "lowerThird") {
    return { ok: false, error: "ローワーサードではありません" };
  }
  const withPrimary = replaceParam(clip, LOWER_THIRD_EFFECT_ID, paramIds.primary, {
    type: "str",
    value: primary,
  });
  if (!withPrimary) {
    return { ok: false, error: "ローワーサードではありません" };
  }
  const next = replaceParam(withPrimary, LOWER_THIRD_EFFECT_ID, paramIds.secondary, {
    type: "str",
    value: secondary,
  });
  if (!next) {
    return { ok: false, error: "ローワーサードではありません" };
  }
  return { ok: true, value: { ...next, name: primary } };
}

function readStringParam(clip: Clip, effectId: string, paramId: string): string | null {
  const effect = (clip.effects ?? []).find((item) => item.effectId === effectId);
  const param = effect?.params.find((item) => item.id === paramId);
  if (!param || param.value.type !== "str") {
    return null;
  }
  return param.value.value;
}

function replaceParam(
  clip: Clip,
  effectId: string,
  paramId: string,
  value: ParamValue,
): Clip | null {
  const effects = clip.effects ?? [];
  if (!effects.some((effect) => effect.effectId === effectId)) {
    return null;
  }
  return {
    ...clip,
    effects: effects.map((effect) => {
      if (effect.effectId !== effectId) {
        return effect;
      }
      return { ...effect, params: replaceOrAppend(effect.params, paramId, value) };
    }),
  };
}

function replaceOrAppend(params: Param[], paramId: string, value: ParamValue): Param[] {
  if (params.some((param) => param.id === paramId)) {
    return params.map((param) => (param.id === paramId ? { ...param, value } : param));
  }
  return [...params, { id: paramId, value }];
}
