import { useEffect, useRef, useState } from "react";
import { computeParade, computeWaveform, type RgbFrame, type ScopeMode } from "./scopes";

export function ScopesView({ frame }: { frame?: RgbFrame }) {
  const [mode, setMode] = useState<ScopeMode>("waveform");
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const context = canvas.getContext("2d");
    if (!context) {
      return;
    }
    if (!frame) {
      context.clearRect(0, 0, canvas.width, canvas.height);
      return;
    }
    const grid = scopeGrid(mode, frame);
    canvas.width = grid.width;
    canvas.height = grid.height;
    const image = context.createImageData(grid.width, grid.height);
    for (let index = 0; index < grid.values.length; index += 1) {
      const column = index % grid.width;
      const tone = Math.round(grid.values[index] * 255);
      const tint = paradeTint(mode, column, frame.width);
      const offset = index * 4;
      image.data[offset] = Math.round(tone * tint.r);
      image.data[offset + 1] = Math.round(tone * tint.g);
      image.data[offset + 2] = Math.round(tone * tint.b);
      image.data[offset + 3] = 255;
    }
    context.putImageData(image, 0, 0);
  }, [frame, mode]);

  return (
    <section className="vc-color__scopes" aria-label="Scopes">
      <div className="vc-color__scope-modes" role="tablist" aria-label="Scope mode">
        <ScopeButton mode="waveform" current={mode} onSelect={setMode} label="Waveform" />
        <ScopeButton mode="parade" current={mode} onSelect={setMode} label="Parade" />
      </div>
      {frame ? (
        <canvas ref={canvasRef} className="vc-color__scope-canvas" />
      ) : (
        <p className="vc-color__empty">No frame for the scopes.</p>
      )}
    </section>
  );
}

function ScopeButton({
  mode,
  current,
  label,
  onSelect,
}: {
  mode: ScopeMode;
  current: ScopeMode;
  label: string;
  onSelect: (mode: ScopeMode) => void;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={mode === current}
      className={mode === current ? "is-active" : undefined}
      onClick={() => onSelect(mode)}
    >
      {label}
    </button>
  );
}

function scopeGrid(mode: ScopeMode, frame: RgbFrame) {
  switch (mode) {
    case "waveform":
      return computeWaveform(frame);
    case "parade":
      return computeParade(frame);
    default: {
      const exhaustive: never = mode;
      return exhaustive;
    }
  }
}

function paradeTint(mode: ScopeMode, column: number, frameWidth: number): { r: number; g: number; b: number } {
  if (mode === "waveform") {
    return { r: 1, g: 1, b: 1 };
  }
  const channel = Math.floor(column / frameWidth);
  if (channel === 0) return { r: 1, g: 0.35, b: 0.3 };
  if (channel === 1) return { r: 0.4, g: 1, b: 0.45 };
  return { r: 0.4, g: 0.65, b: 1 };
}
