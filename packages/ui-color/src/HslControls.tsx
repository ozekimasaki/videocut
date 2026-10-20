import type { EffectInstance } from "@videocut/model";
import { colorParams, readFloat, setFloatParam } from "./grade";

const SLIDERS = [
  { id: colorParams.hue, label: "Hue", min: -180, max: 180, step: 1 },
  { id: colorParams.saturation, label: "Saturation", min: 0, max: 2, step: 0.01 },
  { id: colorParams.lightness, label: "Lightness", min: -1, max: 1, step: 0.01 },
  { id: colorParams.hueCenter, label: "Hue center", min: 0, max: 360, step: 1 },
  { id: colorParams.hueWidth, label: "Hue width", min: 0, max: 360, step: 1 },
  { id: colorParams.satLow, label: "Saturation low", min: 0, max: 1, step: 0.01 },
  { id: colorParams.satHigh, label: "Saturation high", min: 0, max: 1, step: 0.01 },
  { id: colorParams.lumLow, label: "Luma low", min: 0, max: 1, step: 0.01 },
  { id: colorParams.lumHigh, label: "Luma high", min: 0, max: 1, step: 0.01 },
  { id: colorParams.softness, label: "Softness", min: 0, max: 1, step: 0.01 },
] as const;

export function HslControls({
  effect,
  onChange,
}: {
  effect: EffectInstance;
  onChange: (effect: EffectInstance) => void;
}) {
  return (
    <div className="vc-color__hsl">
      {SLIDERS.map((slider) => {
        const value = readFloat(effect, slider.id, fallbackFor(slider.id));
        return (
          <label key={slider.id} className="vc-color__slider">
            <span>
              {slider.label} <span className="vc-color__muted">{value.toFixed(2)}</span>
            </span>
            <input
              type="range"
              min={slider.min}
              max={slider.max}
              step={slider.step}
              value={value}
              aria-label={slider.label}
              onChange={(event) => onChange(setFloatParam(effect, slider.id, Number(event.target.value)))}
            />
          </label>
        );
      })}
    </div>
  );
}

function fallbackFor(id: string): number {
  switch (id) {
    case colorParams.saturation:
    case colorParams.satHigh:
    case colorParams.lumHigh:
      return 1;
    case colorParams.hueWidth:
      return 360;
    default:
      return 0;
  }
}
