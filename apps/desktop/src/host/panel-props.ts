import type { Project, Sequence } from "@videocut/model";
import type { EditCommand } from "./command-stack";
import type { ToolId } from "./types";

export interface PanelHostProps {
  project: Project;
  sequence: Sequence | null;
  sequenceId: string | null;
  tool: ToolId;
  playheadTicks: number;
  selectedClipIds: string[];
  markInTicks: number | null;
  markOutTicks: number | null;
  execute: (command: EditCommand) => void;
  setPlayhead?: (ticks: number) => void;
  setSelectedClipIds?: (ids: string[]) => void;
}

export interface ExportRequest {
  sequenceId: string;
  range: "in-out" | "entire";
  preset: "h264" | "prores" | "wav";
  outputPath: string;
}

export interface SequenceExporter {
  exportSequence: (request: ExportRequest) => Promise<{ ok: boolean; message: string }>;
}
