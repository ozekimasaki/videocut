import type { ReactNode } from "react";
import type { ToolId } from "../host/types";

function Glyph({ children }: { children: ReactNode }) {
  return (
    <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
      {children}
    </svg>
  );
}

const ink = {
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

export function SelectIcon() {
  return (
    <Glyph>
      <path d="M3.2 2.2 3.2 12.2 6.1 9.4 8.1 13.1 9.5 12.4 7.5 8.7 11.4 8.5 Z" fill="currentColor" />
    </Glyph>
  );
}

export function TrackForwardIcon() {
  return (
    <Glyph>
      <path d="M2 4.5h7M2 8h7M2 11.5h7" {...ink} />
      <path d="M10.5 5.2 13.5 8 10.5 10.8" {...ink} />
    </Glyph>
  );
}

export function TrackBackIcon() {
  return (
    <Glyph>
      <path d="M7 4.5h7M7 8h7M7 11.5h7" {...ink} />
      <path d="M5.5 5.2 2.5 8 5.5 10.8" {...ink} />
    </Glyph>
  );
}

export function RippleIcon() {
  return (
    <Glyph>
      <path d="M6 3v10" {...ink} />
      <path d="M9 5c1.4 1 1.4 5 0 6M11.2 3.8c2.2 1.8 2.2 6.6 0 8.4" {...ink} />
    </Glyph>
  );
}

export function RollingIcon() {
  return (
    <Glyph>
      <path d="M6 3.2 3.2 8 6 12.8M10 3.2 12.8 8 10 12.8" {...ink} />
    </Glyph>
  );
}

export function RazorIcon() {
  return (
    <Glyph>
      <path d="M3 13 12.5 3.5" {...ink} />
      <path d="M9.2 3.2h3.2v3.2" {...ink} />
      <path d="M4.2 11.2 6.4 13.4" {...ink} />
    </Glyph>
  );
}

export function SlipIcon() {
  return (
    <Glyph>
      <rect x="3" y="4" width="10" height="8" rx="1" {...ink} />
      <path d="M5.2 8h5.6M5.6 6.4 4.2 8l1.4 1.6M10.4 6.4 11.8 8l-1.4 1.6" {...ink} />
    </Glyph>
  );
}

export function SlideIcon() {
  return (
    <Glyph>
      <rect x="5" y="4.5" width="6" height="7" rx="1" {...ink} />
      <path d="M3.2 8H5M11 8h1.8M2.4 6.6 1.2 8l1.2 1.4M13.6 6.6 14.8 8l-1.2 1.4" {...ink} />
    </Glyph>
  );
}

export function PenIcon() {
  return (
    <Glyph>
      <path d="M9.2 2.8 13.2 6.8 6.2 13.8 2.4 13.6 2.2 9.8 Z" {...ink} />
      <path d="M8.2 3.8 12.2 7.8" {...ink} />
    </Glyph>
  );
}

export function UndoIcon() {
  return (
    <Glyph>
      <path d="M6 4.2 3.2 6.8 6 9.4" {...ink} />
      <path d="M3.6 6.8H9.2A3.6 3.6 0 0 1 9.2 14H7" {...ink} />
    </Glyph>
  );
}

export function RedoIcon() {
  return (
    <Glyph>
      <path d="M10 4.2 12.8 6.8 10 9.4" {...ink} />
      <path d="M12.4 6.8H6.8A3.6 3.6 0 0 0 6.8 14H9" {...ink} />
    </Glyph>
  );
}

export function PlayIcon() {
  return (
    <Glyph>
      <path d="M5 3.2v9.6L12.4 8 Z" fill="currentColor" />
    </Glyph>
  );
}

export function PauseIcon() {
  return (
    <Glyph>
      <path d="M5 3.5v9M11 3.5v9" {...ink} />
    </Glyph>
  );
}

export function StepBackIcon() {
  return (
    <Glyph>
      <path d="M4 3.5v9M12 4.2 6.2 8 12 11.8 Z" {...ink} />
    </Glyph>
  );
}

export function StepForwardIcon() {
  return (
    <Glyph>
      <path d="M12 3.5v9M4 4.2 9.8 8 4 11.8 Z" {...ink} />
    </Glyph>
  );
}

export function ToolIcon({ tool }: { tool: ToolId }) {
  switch (tool) {
    case "select":
      return <SelectIcon />;
    case "trackSelectForward":
      return <TrackForwardIcon />;
    case "trackSelectBackward":
      return <TrackBackIcon />;
    case "ripple":
      return <RippleIcon />;
    case "rolling":
      return <RollingIcon />;
    case "razor":
      return <RazorIcon />;
    case "slip":
      return <SlipIcon />;
    case "slide":
      return <SlideIcon />;
    case "pen":
      return <PenIcon />;
    default: {
      const unknown: never = tool;
      throw new Error(`未知のツール ${String(unknown)}`);
    }
  }
}
