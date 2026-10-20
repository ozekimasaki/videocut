import type { PointerEvent } from "react";
import type { EffectInstance } from "@videocut/model";
import { colorParams, readFloat, setFloatParam } from "./grade";

interface WheelSpec {
  name: string;
  x: string;
  y: string;
  master: string;
}

const WHEELS: readonly WheelSpec[] = [
  {
    name: "Shadows",
    x: colorParams.shadowsX,
    y: colorParams.shadowsY,
    master: colorParams.shadowsMaster,
  },
  {
    name: "Midtones",
    x: colorParams.midtonesX,
    y: colorParams.midtonesY,
    master: colorParams.midtonesMaster,
  },
  {
    name: "Highlights",
    x: colorParams.highlightsX,
    y: colorParams.highlightsY,
    master: colorParams.highlightsMaster,
  },
];

export function ColorWheels({
  effect,
  onChange,
}: {
  effect: EffectInstance;
  onChange: (effect: EffectInstance) => void;
}) {
  return (
    <div className="vc-color__wheels">
      {WHEELS.map((wheel) => (
        <Wheel key={wheel.name} spec={wheel} effect={effect} onChange={onChange} />
      ))}
    </div>
  );
}

function Wheel({
  spec,
  effect,
  onChange,
}: {
  spec: WheelSpec;
  effect: EffectInstance;
  onChange: (effect: EffectInstance) => void;
}) {
  const x = readFloat(effect, spec.x, 0);
  const y = readFloat(effect, spec.y, 0);
  const master = readFloat(effect, spec.master, 0);
  const puck = clampToCircle(x, y);

  function commit(nextX: number, nextY: number) {
    const placed = clampToCircle(nextX, nextY);
    onChange(setFloatParam(setFloatParam(effect, spec.x, placed.x), spec.y, placed.y));
  }

  function onPointer(event: PointerEvent<SVGSVGElement>) {
    event.currentTarget.setPointerCapture(event.pointerId);
    const point = pointerToWheel(event);
    commit(point.x, point.y);
  }

  return (
    <section className="vc-color__wheel" aria-label={`${spec.name} wheel`}>
      <h3>{spec.name}</h3>
      <svg
        viewBox="-1.1 -1.1 2.2 2.2"
        role="application"
        aria-label={`${spec.name} color balance`}
        tabIndex={0}
        onPointerDown={onPointer}
        onPointerMove={(event) => {
          if (event.currentTarget.hasPointerCapture(event.pointerId)) {
            const point = pointerToWheel(event);
            commit(point.x, point.y);
          }
        }}
        onKeyDown={(event) => {
          const step = event.shiftKey ? 0.1 : 0.02;
          if (event.key === "ArrowLeft") commit(x - step, y);
          else if (event.key === "ArrowRight") commit(x + step, y);
          else if (event.key === "ArrowUp") commit(x, y + step);
          else if (event.key === "ArrowDown") commit(x, y - step);
          else return;
          event.preventDefault();
        }}
      >
        <circle cx="0" cy="0" r="1" fill="none" stroke="#5c6770" strokeWidth="0.02" />
        <line x1="-1" y1="0" x2="1" y2="0" stroke="#3a4148" strokeWidth="0.015" />
        <line x1="0" y1="-1" x2="0" y2="1" stroke="#3a4148" strokeWidth="0.015" />
        <circle cx={puck.x} cy={-puck.y} r="0.08" fill={puckColor(puck.x, puck.y)} stroke="#f4f7f7" strokeWidth="0.02" />
      </svg>
      <label className="vc-color__slider">
        Master
        <input
          type="range"
          min={-1}
          max={1}
          step={0.01}
          value={master}
          aria-label={`${spec.name} master`}
          onChange={(event) => onChange(setFloatParam(effect, spec.master, Number(event.target.value)))}
        />
      </label>
      <button
        type="button"
        className="vc-color__reset"
        onClick={() => {
          onChange(
            setFloatParam(setFloatParam(setFloatParam(effect, spec.x, 0), spec.y, 0), spec.master, 0),
          );
        }}
      >
        Reset
      </button>
    </section>
  );
}

function pointerToWheel(event: PointerEvent<SVGSVGElement>): { x: number; y: number } {
  const rect = event.currentTarget.getBoundingClientRect();
  const x = ((event.clientX - rect.left) / rect.width) * 2.2 - 1.1;
  const y = -(((event.clientY - rect.top) / rect.height) * 2.2 - 1.1);
  return { x, y };
}

function clampToCircle(x: number, y: number): { x: number; y: number } {
  const length = Math.hypot(x, y);
  if (length <= 1) {
    return { x, y };
  }
  return { x: x / length, y: y / length };
}

function puckColor(x: number, y: number): string {
  if (Math.hypot(x, y) < 0.001) {
    return "#d5dbdb";
  }
  const hue = (Math.atan2(y, x) * 180) / Math.PI;
  return `hsl(${hue} 70% 62%)`;
}
