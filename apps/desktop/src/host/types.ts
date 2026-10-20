export type ToolId =
  | "select"
  | "trackSelectForward"
  | "trackSelectBackward"
  | "ripple"
  | "rolling"
  | "razor"
  | "slip"
  | "slide"
  | "pen";

export interface Shuttle {
  monitor: "source" | "program";
  direction: -1 | 1;
  speed: number;
}

export interface SessionState {
  activeSequenceId: string | null;
  tool: ToolId;
  selectedClipIds: string[];
  playheadTicks: number;
  sourceMediaId: string | null;
  sourceClipId: string | null;
  sourcePlayheadTicks: number;
  sourceMarkIn: number | null;
  sourceMarkOut: number | null;
  programMarkIn: number | null;
  programMarkOut: number | null;
  focusedMonitor: "source" | "program";
  playing: Shuttle | null;
  projectPath: string | null;
  message: string;
}

export interface ToolSpec {
  id: ToolId;
  label: string;
  shortcut: string;
}

export const TOOLS: readonly ToolSpec[] = [
  { id: "select", label: "選択", shortcut: "V" },
  { id: "trackSelectForward", label: "トラック選択（前方）", shortcut: "A" },
  { id: "trackSelectBackward", label: "トラック選択（後方）", shortcut: "Shift+A" },
  { id: "ripple", label: "リップル", shortcut: "B" },
  { id: "rolling", label: "ローリング", shortcut: "N" },
  { id: "razor", label: "レーザー", shortcut: "C" },
  { id: "slip", label: "スリップ", shortcut: "Y" },
  { id: "slide", label: "スライド", shortcut: "U" },
  { id: "pen", label: "ペン", shortcut: "P" },
];

export function toolSpec(id: ToolId): ToolSpec {
  const spec = TOOLS.find((tool) => tool.id === id);
  if (!spec) throw new Error(`未知のツール ${id}`);
  return spec;
}
