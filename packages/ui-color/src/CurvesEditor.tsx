import { useState, type MouseEvent } from "react";
import type { EffectInstance } from "@videocut/model";
import {
  dragCurvePoint,
  formatCurve,
  insertCurvePoint,
  parseCurve,
  removeCurvePoint,
  sampleCurve,
  type CurvePoint,
} from "./curves";
import { colorParams, readString, setStringParam } from "./grade";

const CHANNELS = [
  { id: colorParams.master, label: "Master" },
  { id: colorParams.red, label: "Red" },
  { id: colorParams.green, label: "Green" },
  { id: colorParams.blue, label: "Blue" },
] as const;

type ChannelId = (typeof CHANNELS)[number]["id"];

export function CurvesEditor({
  effect,
  onChange,
}: {
  effect: EffectInstance;
  onChange: (effect: EffectInstance) => void;
}) {
  const [channel, setChannel] = useState<ChannelId>(colorParams.master);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const source = readString(effect, channel, "0,0;1,1");
  const parsed = parseCurve(source);

  function write(points: readonly CurvePoint[]) {
    onChange(setStringParam(effect, channel, formatCurve(points)));
  }

  return (
    <div className="vc-color__curves">
      <div className="vc-color__channels" role="tablist" aria-label="Curve channel">
        {CHANNELS.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={item.id === channel}
            className={item.id === channel ? "is-active" : undefined}
            onClick={() => setChannel(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>
      {parsed.ok ? (
        <CurveGraph
          points={parsed.points}
          channel={channel}
          dragIndex={dragIndex}
          onDragIndex={setDragIndex}
          onPoints={write}
        />
      ) : (
        <p className="vc-color__empty">{parsed.message}</p>
      )}
    </div>
  );
}

function CurveGraph({
  points,
  channel,
  dragIndex,
  onDragIndex,
  onPoints,
}: {
  points: CurvePoint[];
  channel: ChannelId;
  dragIndex: number | null;
  onDragIndex: (index: number | null) => void;
  onPoints: (points: readonly CurvePoint[]) => void;
}) {
  const samples = Array.from({ length: 33 }, (_, index) => {
    const x = index / 32;
    return `${x},${1 - sampleCurve(points, x)}`;
  }).join(" ");

  function at(event: MouseEvent<SVGSVGElement>): CurvePoint {
    const rect = event.currentTarget.getBoundingClientRect();
    return {
      x: (event.clientX - rect.left) / rect.width,
      y: 1 - (event.clientY - rect.top) / rect.height,
    };
  }

  return (
    <svg
      className="vc-color__curve"
      viewBox="0 0 1 1"
      role="img"
      aria-label={`${channel} curve`}
      onPointerMove={(event) => {
        if (dragIndex === null || !event.currentTarget.hasPointerCapture(event.pointerId)) {
          return;
        }
        const point = at(event);
        onPoints(dragCurvePoint(points, dragIndex, point.x, point.y));
      }}
      onPointerUp={() => onDragIndex(null)}
      onDoubleClick={(event) => {
        const point = at(event);
        onPoints(insertCurvePoint(points, point.x, point.y));
      }}
    >
      <line x1="0" y1="1" x2="1" y2="0" stroke="#3a4148" strokeWidth="0.008" />
      <polyline fill="none" stroke={strokeFor(channel)} strokeWidth="0.012" points={samples} />
      {points.map((point, index) => (
        <circle
          key={`${point.x}-${index}`}
          cx={point.x}
          cy={1 - point.y}
          r="0.025"
          fill="#f4f7f7"
          stroke={strokeFor(channel)}
          strokeWidth="0.01"
          onPointerDown={(event) => {
            event.stopPropagation();
            event.currentTarget.ownerSVGElement?.setPointerCapture(event.pointerId);
            onDragIndex(index);
          }}
          onDoubleClick={(event) => {
            event.stopPropagation();
            onPoints(removeCurvePoint(points, index));
          }}
        />
      ))}
    </svg>
  );
}

function strokeFor(channel: ChannelId): string {
  switch (channel) {
    case colorParams.master:
      return "#d5dbdb";
    case colorParams.red:
      return "#e06a5c";
    case colorParams.green:
      return "#6fbf73";
    case colorParams.blue:
      return "#6aa6e0";
    default: {
      const exhaustive: never = channel;
      return exhaustive;
    }
  }
}
