import type { EffectInstance, Interp, Param, ParamValue } from "@videocut/model";

import { createEffect } from "./catalog";
import { evaluateParam } from "./evaluate";
import { cloneValue } from "./values";

export function setEffectEnabled(
  effects: readonly EffectInstance[],
  id: string,
  enabled: boolean,
): EffectInstance[] {
  return effects.map((effect) => (effect.id === id ? { ...effect, enabled } : effect));
}

export function removeEffect(effects: readonly EffectInstance[], id: string): EffectInstance[] {
  return effects.filter((effect) => effect.id !== id);
}

export function moveEffect(
  effects: readonly EffectInstance[],
  id: string,
  direction: -1 | 1,
): EffectInstance[] {
  const index = effects.findIndex((effect) => effect.id === id);
  const target = index + direction;
  if (index < 0 || target < 0 || target >= effects.length) return [...effects];
  const next = [...effects];
  const [item] = next.splice(index, 1);
  if (!item) return [...effects];
  next.splice(target, 0, item);
  return next;
}

export function addEffect(effects: readonly EffectInstance[], effectId: string): EffectInstance[] {
  const created = createEffect(effectId);
  if (!created) return [...effects];
  return [...effects, created];
}

export function setParamValue(
  effects: readonly EffectInstance[],
  effectId: string,
  paramId: string,
  value: ParamValue,
  localTick: number,
): EffectInstance[] {
  return effects.map((effect) => {
    if (effect.id !== effectId) return effect;
    const exists = effect.params.some((param) => param.id === paramId);
    const params = exists
      ? effect.params.map((param) =>
          param.id === paramId ? writeParam(param, value, localTick) : param,
        )
      : [...effect.params, writeParam({ id: paramId, value, keyframes: [] }, value, localTick)];
    return { ...effect, params };
  });
}

export function toggleKeyframe(
  effects: readonly EffectInstance[],
  effectId: string,
  paramId: string,
  localTick: number,
): EffectInstance[] {
  return effects.map((effect) => {
    if (effect.id !== effectId) return effect;
    return {
      ...effect,
      params: effect.params.map((param) => {
        if (param.id !== paramId) return param;
        const keys = param.keyframes ?? [];
        const existing = keys.find((key) => key.timeTicks === localTick);
        if (existing) {
          const keyframes = keys.filter((key) => key.timeTicks !== localTick);
          if (keyframes.length === 0) {
            return { ...param, value: cloneValue(existing.value), keyframes };
          }
          return { ...param, keyframes };
        }
        const keyframes = [
          ...keys,
          { timeTicks: localTick, value: evaluateParam(param, localTick), interp: "linear" as const },
        ].sort((a, b) => a.timeTicks - b.timeTicks);
        return { ...param, keyframes };
      }),
    };
  });
}

export function setKeyframeInterp(
  effects: readonly EffectInstance[],
  effectId: string,
  paramId: string,
  timeTicks: number,
  interp: Interp,
): EffectInstance[] {
  return effects.map((effect) => {
    if (effect.id !== effectId) return effect;
    return {
      ...effect,
      params: effect.params.map((param) => {
        if (param.id !== paramId) return param;
        return {
          ...param,
          keyframes: (param.keyframes ?? []).map((key) =>
            key.timeTicks === timeTicks ? { ...key, interp } : key,
          ),
        };
      }),
    };
  });
}

export function deleteKeyframe(
  effects: readonly EffectInstance[],
  effectId: string,
  paramId: string,
  timeTicks: number,
): EffectInstance[] {
  return effects.map((effect) => {
    if (effect.id !== effectId) return effect;
    return {
      ...effect,
      params: effect.params.map((param) => {
        if (param.id !== paramId) return param;
        const keys = param.keyframes ?? [];
        const existing = keys.find((key) => key.timeTicks === timeTicks);
        const keyframes = keys.filter((key) => key.timeTicks !== timeTicks);
        if (existing && keyframes.length === 0) {
          return { ...param, value: cloneValue(existing.value), keyframes };
        }
        return { ...param, keyframes };
      }),
    };
  });
}

function writeParam(param: Param, value: ParamValue, localTick: number): Param {
  const keys = param.keyframes ?? [];
  if (keys.length === 0) return { ...param, value: cloneValue(value) };
  const index = keys.findIndex((key) => key.timeTicks === localTick);
  if (index >= 0) {
    const keyframes = keys.map((key, position) =>
      position === index ? { ...key, value: cloneValue(value) } : key,
    );
    return { ...param, keyframes };
  }
  const keyframes = [...keys, { timeTicks: localTick, value: cloneValue(value), interp: "linear" as const }].sort(
    (a, b) => a.timeTicks - b.timeTicks,
  );
  return { ...param, keyframes };
}
