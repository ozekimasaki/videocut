import { useState } from "react";
import type { ChangeEvent } from "react";
import {
  effectIds,
  TICKS_PER_SECOND,
  type Clip,
  type EffectInstance,
  type Param,
  type ParamValue,
  type Transition,
} from "@videocut/model";

import { effectLabel, effectSpecs, paramSpec } from "./catalog";
import {
  addEffect,
  deleteKeyframe,
  moveEffect,
  removeEffect,
  setEffectEnabled,
  setKeyframeInterp,
  setParamValue,
  toggleKeyframe,
} from "./edit";
import {
  blendWeights,
  evaluateParam,
  isInterp,
  isTransitionAlign,
  localTicks,
  transitionProgress,
} from "./evaluate";
import { formatBlend, formatParamValue } from "./format";
import { fromDisplay, toDisplay, type DisplayScale } from "./values";

export interface EffectControlsProps {
  clip: Clip;
  playheadTicks: number;
  onEffectsChange: (effects: EffectInstance[]) => void;
  transition?: Transition | null;
  onTransitionChange?: (transition: Transition) => void;
}

export function EffectControls({
  clip,
  playheadTicks,
  onEffectsChange,
  transition,
  onTransitionChange,
}: EffectControlsProps) {
  const effects = clip.effects ?? [];
  const localTick = localTicks(clip.timelineIn, playheadTicks);
  const filters = effectSpecs().filter((spec) => spec.kind === "filter");
  const [pendingId, setPendingId] = useState(filters[0]?.effectId ?? "");

  return (
    <section className="vc-effects" aria-label="Effect Controls" data-testid="effect-controls">
      <style>{PANEL_CSS}</style>
      <header className="vc-effects__header">
        <h2>Effect Controls</h2>
        <p>
          {clip.name}
          {clip.kind === "adjustment" ? " · Adjustment layer" : ""}
        </p>
        <p className="vc-effects__meta">Local {localTick} ticks</p>
      </header>

      <div className="vc-effects__add">
        <label>
          Add effect
          <select
            aria-label="Add effect"
            value={pendingId}
            onChange={(event) => setPendingId(event.target.value)}
          >
            {filters.map((spec) => (
              <option key={spec.effectId} value={spec.effectId}>
                {spec.label}
              </option>
            ))}
          </select>
        </label>
        <button type="button" onClick={() => onEffectsChange(addEffect(effects, pendingId))}>
          Add
        </button>
      </div>

      {effects.length === 0 ? <p className="vc-effects__empty">No effects on this clip</p> : null}

      <div className="vc-effects__stack">
        {effects.map((effect, index) => (
          <EffectCard
            key={effect.id}
            effect={effect}
            title={stackedLabel(effects, index)}
            localTick={localTick}
            canMoveUp={index > 0}
            canMoveDown={index < effects.length - 1}
            onEffectsChange={(next) => onEffectsChange(next)}
            effects={effects}
          />
        ))}
      </div>

      {transition ? (
        <TransitionCard
          transition={transition}
          playheadTicks={playheadTicks}
          onTransitionChange={onTransitionChange}
        />
      ) : null}
    </section>
  );
}

function EffectCard({
  effect,
  title,
  localTick,
  canMoveUp,
  canMoveDown,
  effects,
  onEffectsChange,
}: {
  effect: EffectInstance;
  title: string;
  localTick: number;
  canMoveUp: boolean;
  canMoveDown: boolean;
  effects: EffectInstance[];
  onEffectsChange: (effects: EffectInstance[]) => void;
}) {
  return (
    <fieldset className="vc-effects__card">
      <legend>
        <label>
          <input
            type="checkbox"
            aria-label={`${title} enabled`}
            checked={effect.enabled}
            onChange={(event) =>
              onEffectsChange(setEffectEnabled(effects, effect.id, event.target.checked))
            }
          />
          {title}
        </label>
        <span className="vc-effects__card-actions">
          <button
            type="button"
            aria-label={`Move ${title} up`}
            disabled={!canMoveUp}
            onClick={() => onEffectsChange(moveEffect(effects, effect.id, -1))}
          >
            Up
          </button>
          <button
            type="button"
            aria-label={`Move ${title} down`}
            disabled={!canMoveDown}
            onClick={() => onEffectsChange(moveEffect(effects, effect.id, 1))}
          >
            Down
          </button>
          <button
            type="button"
            aria-label={`Remove ${title}`}
            onClick={() => onEffectsChange(removeEffect(effects, effect.id))}
          >
            Remove
          </button>
        </span>
      </legend>
      {effect.params.map((param) => (
        <ParamRow
          key={param.id}
          effect={effect}
          param={param}
          localTick={localTick}
          effects={effects}
          onEffectsChange={onEffectsChange}
        />
      ))}
    </fieldset>
  );
}

function ParamRow({
  effect,
  param,
  localTick,
  effects,
  onEffectsChange,
}: {
  effect: EffectInstance;
  param: Param;
  localTick: number;
  effects: EffectInstance[];
  onEffectsChange: (effects: EffectInstance[]) => void;
}) {
  const spec = paramSpec(effect.effectId, param.id);
  const label = spec?.label ?? param.id;
  const evaluated = evaluateParam(param, localTick);
  const hasKey = (param.keyframes ?? []).some((key) => key.timeTicks === localTick);
  const write = (value: ParamValue) =>
    onEffectsChange(setParamValue(effects, effect.id, param.id, value, localTick));

  return (
    <div className="vc-effects__param">
      <div className="vc-effects__param-head">
        <span>{label}</span>
        <output data-testid={`evaluated-${effect.id}-${param.id}`}>{formatParamValue(param.id, evaluated)}</output>
        <button
          type="button"
          aria-pressed={hasKey}
          aria-label={hasKey ? `Remove keyframe for ${label}` : `Add keyframe for ${label}`}
          onClick={() => onEffectsChange(toggleKeyframe(effects, effect.id, param.id, localTick))}
        >
          {hasKey ? "Keyed" : "Key"}
        </button>
      </div>
      <ValueEditor label={label} value={evaluated} display={spec?.display ?? "number"} min={spec?.min} max={spec?.max} onChange={write} />
      {(param.keyframes ?? []).map((key) => (
        <div key={key.timeTicks} className="vc-effects__key">
          <span>{key.timeTicks}</span>
          <select
            aria-label={`Interpolation at ${key.timeTicks} for ${label}`}
            value={key.interp}
            onChange={(event) => {
              if (!isInterp(event.target.value)) return;
              onEffectsChange(
                setKeyframeInterp(effects, effect.id, param.id, key.timeTicks, event.target.value),
              );
            }}
          >
            <option value="linear">Linear</option>
            <option value="hold">Hold</option>
            <option value="bezier">Bezier</option>
          </select>
          <button
            type="button"
            aria-label={`Delete keyframe at ${key.timeTicks} for ${label}`}
            onClick={() => onEffectsChange(deleteKeyframe(effects, effect.id, param.id, key.timeTicks))}
          >
            Delete
          </button>
        </div>
      ))}
    </div>
  );
}

function ValueEditor({
  label,
  value,
  display,
  min,
  max,
  onChange,
}: {
  label: string;
  value: ParamValue;
  display: DisplayScale;
  min?: number;
  max?: number;
  onChange: (value: ParamValue) => void;
}) {
  switch (value.type) {
    case "float":
    case "int":
      return (
        <NumberField
          label={`${label} value`}
          stored={value.value}
          display={value.type === "int" ? "number" : display}
          min={min}
          max={max}
          integer={value.type === "int"}
          onStored={(next) =>
            onChange(value.type === "int" ? { type: "int", value: next } : { type: "float", value: next })
          }
        />
      );
    case "bool":
      return (
        <label>
          <input
            type="checkbox"
            aria-label={`${label} value`}
            checked={value.value}
            onChange={(event) => onChange({ type: "bool", value: event.target.checked })}
          />
          {value.value ? "On" : "Off"}
        </label>
      );
    case "point":
      return (
        <span className="vc-effects__pair">
          <NumberField
            label={`${label} x`}
            stored={value.value.x}
            display={display}
            min={min}
            max={max}
            onStored={(x) => onChange({ type: "point", value: { x, y: value.value.y } })}
          />
          <NumberField
            label={`${label} y`}
            stored={value.value.y}
            display={display}
            min={min}
            max={max}
            onStored={(y) => onChange({ type: "point", value: { x: value.value.x, y } })}
          />
        </span>
      );
    case "color":
      return (
        <span className="vc-effects__pair">
          {(["r", "g", "b", "a"] as const).map((channel) => (
            <NumberField
              key={channel}
              label={`${label} ${channel}`}
              stored={value.value[channel]}
              display="number"
              min={0}
              max={1}
              onStored={(channelValue) =>
                onChange({ type: "color", value: { ...value.value, [channel]: channelValue } })
              }
            />
          ))}
        </span>
      );
    case "str":
      return (
        <input
          aria-label={`${label} value`}
          type="text"
          value={value.value}
          onChange={(event) => onChange({ type: "str", value: event.target.value })}
        />
      );
    default: {
      const never: never = value;
      return never;
    }
  }
}

function NumberField({
  label,
  stored,
  display,
  min,
  max,
  integer = false,
  onStored,
}: {
  label: string;
  stored: number;
  display: DisplayScale;
  min?: number;
  max?: number;
  integer?: boolean;
  onStored: (value: number) => void;
}) {
  const shown = toDisplay(display, stored);
  return (
    <input
      aria-label={label}
      type="number"
      step={integer ? 1 : display === "percent" ? 0.1 : 0.01}
      min={min === undefined ? undefined : toDisplay(display, min)}
      max={max === undefined ? undefined : toDisplay(display, max)}
      value={Number.isFinite(shown) ? shown : 0}
      onChange={(event: ChangeEvent<HTMLInputElement>) => {
        const next = Number(event.target.value);
        if (!Number.isFinite(next)) return;
        onStored(integer ? Math.round(next) : fromDisplay(display, next));
      }}
    />
  );
}

function TransitionCard({
  transition,
  playheadTicks,
  onTransitionChange,
}: {
  transition: Transition;
  playheadTicks: number;
  onTransitionChange?: (transition: Transition) => void;
}) {
  const progress = transitionProgress(transition, playheadTicks);
  const blend = progress === null ? null : blendWeights(transition.effectId, progress);
  const seconds = transition.durationTicks / TICKS_PER_SECOND;
  const readOnly = onTransitionChange === undefined;

  return (
    <fieldset className="vc-effects__card">
      <legend>{effectLabel(transition.effectId)}</legend>
      <label>
        Duration (seconds)
        <input
          aria-label="Transition duration"
          type="number"
          min={0}
          step={0.01}
          disabled={readOnly}
          value={seconds}
          onChange={(event) => {
            const next = Number(event.target.value);
            if (!Number.isFinite(next) || !onTransitionChange) return;
            onTransitionChange({
              ...transition,
              durationTicks: Math.max(0, Math.round(next * TICKS_PER_SECOND)),
            });
          }}
        />
      </label>
      <label>
        Alignment
        <select
          aria-label="Transition alignment"
          disabled={readOnly}
          value={transition.alignment}
          onChange={(event) => {
            if (!isTransitionAlign(event.target.value) || !onTransitionChange) return;
            onTransitionChange({ ...transition, alignment: event.target.value });
          }}
        >
          <option value="start">Start</option>
          <option value="center">Center</option>
          <option value="end">End</option>
        </select>
      </label>
      {blend ? (
        <p data-testid="transition-blend">
          Outgoing {formatBlend(blend.outgoing)} · Incoming {formatBlend(blend.incoming)}
          {blend.dip > 0 ? ` · Dip ${formatBlend(blend.dip)} ${blend.dipColor}` : ""}
          {transition.effectId === effectIds.wipe ? ` · Edge ${formatBlend(blend.wipeEdge)}` : ""}
        </p>
      ) : (
        <p data-testid="transition-blend">Outside transition</p>
      )}
    </fieldset>
  );
}

function stackedLabel(effects: readonly EffectInstance[], index: number): string {
  const effect = effects[index];
  if (!effect) return "";
  const base = effectLabel(effect.effectId);
  const count = effects.filter((item) => item.effectId === effect.effectId).length;
  if (count < 2) return base;
  const ordinal = effects.slice(0, index + 1).filter((item) => item.effectId === effect.effectId).length;
  return `${base} ${ordinal}`;
}

const PANEL_CSS = `
.vc-effects {
  box-sizing: border-box;
  background: #1c1c1c;
  color: #e6e6e6;
  font: 12px/1.4 system-ui, sans-serif;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 240px;
}
.vc-effects *, .vc-effects *::before, .vc-effects *::after { box-sizing: border-box; }
.vc-effects h2 { margin: 0; font-size: 13px; font-weight: 600; letter-spacing: 0.01em; }
.vc-effects p { margin: 0; }
.vc-effects__meta, .vc-effects__empty { color: #9a9a9a; }
.vc-effects__header { display: flex; flex-direction: column; gap: 2px; }
.vc-effects__add, .vc-effects__param-head, .vc-effects__card legend, .vc-effects__key, .vc-effects__pair {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}
.vc-effects__card {
  margin: 0;
  border: 1px solid #3a3a3a;
  border-radius: 6px;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.vc-effects__card legend { padding: 0 4px; }
.vc-effects__card-actions { margin-left: auto; display: flex; gap: 4px; }
.vc-effects button, .vc-effects input, .vc-effects select {
  background: #2a2a2a;
  color: inherit;
  border: 1px solid #4a4a4a;
  border-radius: 4px;
  padding: 2px 6px;
}
.vc-effects button:disabled { opacity: 0.4; }
.vc-effects input[type="checkbox"] { accent-color: #8fb3c9; }
.vc-effects__param-head output { margin-left: auto; color: #c9d7e0; }
`;
