import { useEffect, useState } from "react";
import type { EffectInstance } from "@videocut/model";
import { ColorWheels } from "./ColorWheels";
import { CurvesEditor } from "./CurvesEditor";
import { HslControls } from "./HslControls";
import { LutControls } from "./LutControls";
import { ScopesView } from "./ScopesView";
import {
  colorToolLabel,
  createColorWheelsEffect,
  createCurvesEffect,
  createHslEffect,
  createLutEffect,
  isColorEffect,
  toolForEffect,
  type ColorTool,
} from "./grade";
import type { RgbFrame } from "./scopes";
import { COLOR_PANEL_CSS } from "./styles";

export interface ColorPanelProps {
  effects: EffectInstance[];
  activeEffectId?: string;
  frame?: RgbFrame;
  onChange: (effect: EffectInstance) => void;
  onSelect?: (effectId: string) => void;
  onAdd?: (effect: EffectInstance) => void;
}

const TOOLS: readonly ColorTool[] = ["wheels", "curves", "hsl", "lut"];

export function ColorPanel({
  effects,
  activeEffectId,
  frame,
  onChange,
  onSelect,
  onAdd,
}: ColorPanelProps) {
  const colorEffects = effects.filter((effect) => isColorEffect(effect.effectId));
  const initialTool = toolForEffect(colorEffects[0]?.effectId ?? "") ?? "wheels";
  const [tool, setTool] = useState<ColorTool>(initialTool);

  useEffect(() => {
    if (!activeEffectId) {
      return;
    }
    const next = toolForEffect(effects.find((effect) => effect.id === activeEffectId)?.effectId ?? "");
    if (next) {
      setTool(next);
    }
  }, [activeEffectId, effects]);

  const selected =
    colorEffects.find(
      (effect) => effect.id === activeEffectId && toolForEffect(effect.effectId) === tool,
    ) ??
    colorEffects.find((effect) => toolForEffect(effect.effectId) === tool) ??
    null;

  function selectTool(next: ColorTool) {
    setTool(next);
    const match = colorEffects.find((effect) => toolForEffect(effect.effectId) === next);
    if (match) {
      onSelect?.(match.id);
    }
  }

  return (
    <section className="vc-color" aria-label="Color">
      <style>{COLOR_PANEL_CSS}</style>
      <header className="vc-color__header">
        <h2>Color</h2>
      </header>
      {colorEffects.length > 0 ? (
        <div className="vc-color__chips" role="listbox" aria-label="Color effects">
          {colorEffects.map((effect) => {
            const effectTool = toolForEffect(effect.effectId);
            const label = effectTool ? colorToolLabel(effectTool) : effect.effectId;
            const active = effect.id === selected?.id;
            return (
              <button
                key={effect.id}
                type="button"
                role="option"
                aria-selected={active}
                className={active ? "vc-color__chip is-active" : "vc-color__chip"}
                onClick={() => {
                  if (effectTool) {
                    setTool(effectTool);
                  }
                  onSelect?.(effect.id);
                }}
              >
                {label}
              </button>
            );
          })}
        </div>
      ) : (
        <p className="vc-color__empty">No color effects on this clip.</p>
      )}
      <div className="vc-color__tabs" role="tablist" aria-label="Color tools">
        {TOOLS.map((item) => (
          <button
            key={item}
            type="button"
            role="tab"
            aria-selected={item === tool}
            className={item === tool ? "vc-color__tab is-active" : "vc-color__tab"}
            onClick={() => selectTool(item)}
          >
            {colorToolLabel(item)}
          </button>
        ))}
      </div>
      <div className="vc-color__editor">
        {selected ? (
          <Editor tool={tool} effect={selected} onChange={onChange} />
        ) : (
          <p className="vc-color__empty">This clip has no {colorToolLabel(tool).toLowerCase()} effect.</p>
        )}
        {onAdd ? (
          <div className="vc-color__add">
            <button type="button" onClick={() => onAdd(createColorWheelsEffect())}>
              Add wheels
            </button>
            <button type="button" onClick={() => onAdd(createCurvesEffect())}>
              Add curves
            </button>
            <button type="button" onClick={() => onAdd(createHslEffect())}>
              Add HSL
            </button>
            <button type="button" onClick={() => onAdd(createLutEffect())}>
              Add LUT
            </button>
          </div>
        ) : null}
      </div>
      <ScopesView frame={frame} />
    </section>
  );
}

function Editor({
  tool,
  effect,
  onChange,
}: {
  tool: ColorTool;
  effect: EffectInstance;
  onChange: (effect: EffectInstance) => void;
}) {
  switch (tool) {
    case "wheels":
      return <ColorWheels effect={effect} onChange={onChange} />;
    case "curves":
      return <CurvesEditor effect={effect} onChange={onChange} />;
    case "hsl":
      return <HslControls effect={effect} onChange={onChange} />;
    case "lut":
      return <LutControls effect={effect} onChange={onChange} />;
    default: {
      const exhaustive: never = tool;
      return exhaustive;
    }
  }
}
