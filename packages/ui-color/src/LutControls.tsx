import type { ChangeEvent } from "react";
import type { EffectInstance } from "@videocut/model";
import { inspectCube } from "./cube";
import { colorParams, readFloat, readString, setFloatParam, setStringParam } from "./grade";

export function LutControls({
  effect,
  onChange,
}: {
  effect: EffectInstance;
  onChange: (effect: EffectInstance) => void;
}) {
  const cube = readString(effect, colorParams.cube, "");
  const intensity = readFloat(effect, colorParams.intensity, 1);
  const inspection = inspectCube(cube);
  const status = inspection.ok
    ? inspection.kind === "identity"
      ? "Identity (no LUT)"
      : `${inspection.kind.toUpperCase()} ${inspection.size}`
    : inspection.message;

  function onFile(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    if (!file) {
      return;
    }
    const reader = new FileReader();
    reader.onload = () => {
      const text = typeof reader.result === "string" ? reader.result : "";
      onChange(setStringParam(effect, colorParams.cube, text));
    };
    reader.readAsText(file);
  }

  return (
    <div className="vc-color__lut">
      <label className="vc-color__slider">
        .cube LUT
        <textarea
          value={cube}
          spellCheck={false}
          aria-label="Cube LUT source"
          onChange={(event) => onChange(setStringParam(effect, colorParams.cube, event.target.value))}
        />
      </label>
      <label className="vc-color__slider">
        Load .cube
        <input type="file" accept=".cube,text/plain" aria-label="Load cube LUT" onChange={onFile} />
      </label>
      <p className="vc-color__muted">{status}</p>
      <label className="vc-color__slider">
        <span>
          Intensity <span className="vc-color__muted">{intensity.toFixed(2)}</span>
        </span>
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={intensity}
          aria-label="LUT intensity"
          onChange={(event) => onChange(setFloatParam(effect, colorParams.intensity, Number(event.target.value)))}
        />
      </label>
    </div>
  );
}
