import { TICKS_PER_SECOND } from "@videocut/model";
import type { Rational, Sequence } from "@videocut/model";

export const EXPORT_PRESETS = [
  { id: "h264", label: "H.264", extension: ".mp4" },
  { id: "prores", label: "ProRes", extension: ".mov" },
  { id: "wav", label: "WAV", extension: ".wav" },
] as const;

export type ExportPresetId = (typeof EXPORT_PRESETS)[number]["id"];

export interface ExportPreset {
  id: ExportPresetId;
  label: string;
  extension: ".mp4" | ".mov" | ".wav";
}

export type ExportRangeMode = "inOut" | "entire";

export interface ExportRange {
  mode: ExportRangeMode;
  /** Inclusive start, in timeline ticks. Required when mode is `inOut`. */
  inTicks?: number;
  /** Exclusive end, in timeline ticks. Required when mode is `inOut`. */
  outTicks?: number;
}

export interface ExportRequest {
  sequenceId: string;
  preset: ExportPresetId;
  range: {
    mode: ExportRangeMode;
    inTicks: number;
    outTicks: number;
  };
  outputPath: string;
}

/**
 * Shells pass the real media exporter here. The panel does not encode media.
 */
export interface MediaExporter {
  exportSequence(request: ExportRequest): Promise<void>;
}

export interface ExportMarks {
  inTicks: number | null;
  outTicks: number | null;
}

export interface ExportFormState {
  sequenceId: string;
  preset: ExportPresetId;
  rangeMode: ExportRangeMode;
  outputPath: string;
}

export type ExportFormResult =
  | { ok: true; request: ExportRequest }
  | { ok: false; message: string };

export class ExportValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ExportValidationError";
  }
}

const KNOWN_EXTENSIONS = [".mp4", ".mov", ".wav"] as const;
const INVALID_FILE_CHARS = /[\\/:*?"<>|]/g;

export function isExportPresetId(value: string): value is ExportPresetId {
  switch (value) {
    case "h264":
    case "prores":
    case "wav":
      return true;
    default:
      return false;
  }
}

export function presetExtension(preset: ExportPresetId): ".mp4" | ".mov" | ".wav" {
  switch (preset) {
    case "h264":
      return ".mp4";
    case "prores":
      return ".mov";
    case "wav":
      return ".wav";
    default: {
      const unknown: never = preset;
      throw new ExportValidationError(`Unknown preset: ${String(unknown)}`);
    }
  }
}

export function sequenceEndTicks(sequence: Sequence): number {
  let end = 0;
  const tracks = [...sequence.videoTracks, ...sequence.audioTracks];
  for (const track of tracks) {
    for (const clip of track.clips) {
      if (clip.timelineOut > end) end = clip.timelineOut;
    }
  }
  return end;
}

/** Non-drop timecode. Frame boundaries follow the sequence frame rate in ticks. */
export function formatTimecode(ticks: number, fps: Rational): string {
  if (fps.num <= 0 || fps.den <= 0) {
    throw new ExportValidationError("Frame rate is invalid");
  }
  const frameTicks = Math.round((TICKS_PER_SECOND * fps.den) / fps.num);
  if (frameTicks <= 0) throw new ExportValidationError("Frame rate is invalid");
  const safeTicks = Number.isFinite(ticks) ? Math.max(0, Math.trunc(ticks)) : 0;
  const frames = Math.floor(safeTicks / frameTicks);
  const framesPerSecond = Math.max(1, Math.round(fps.num / fps.den));
  const frame = frames % framesPerSecond;
  const totalSeconds = Math.floor(frames / framesPerSecond);
  const seconds = totalSeconds % 60;
  const minutes = Math.floor(totalSeconds / 60) % 60;
  const hours = Math.floor(totalSeconds / 3600);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}:${pad(frame)}`;
}

export function resolveExportRange(
  sequence: Sequence,
  range: ExportRange,
): ExportRequest["range"] {
  switch (range.mode) {
    case "entire": {
      const outTicks = sequenceEndTicks(sequence);
      if (outTicks <= 0) throw new ExportValidationError("Sequence has no duration");
      return { mode: "entire", inTicks: 0, outTicks };
    }
    case "inOut": {
      if (range.inTicks == null || range.outTicks == null) {
        throw new ExportValidationError("In and out points are required");
      }
      if (!Number.isInteger(range.inTicks) || !Number.isInteger(range.outTicks)) {
        throw new ExportValidationError("In and out points must be whole ticks");
      }
      if (range.inTicks < 0) throw new ExportValidationError("In point cannot be negative");
      if (range.outTicks <= range.inTicks) {
        throw new ExportValidationError("Out point must be after the in point");
      }
      return { mode: "inOut", inTicks: range.inTicks, outTicks: range.outTicks };
    }
    default: {
      const unknown: never = range.mode;
      throw new ExportValidationError(`Unknown range: ${String(unknown)}`);
    }
  }
}

export function withPresetExtension(outputPath: string, preset: ExportPresetId): string {
  const extension = presetExtension(preset);
  const trimmed = outputPath.trim();
  if (!trimmed) return "";
  const lower = trimmed.toLowerCase();
  for (const known of KNOWN_EXTENSIONS) {
    if (lower.endsWith(known)) return trimmed.slice(0, -known.length) + extension;
  }
  return `${trimmed}${extension}`;
}

export function suggestOutputPath(
  sequenceName: string,
  preset: ExportPresetId,
  directory?: string,
): string {
  const base = sequenceName.replace(INVALID_FILE_CHARS, "_").trim() || "sequence";
  const file = `${base}${presetExtension(preset)}`;
  const dir = directory?.trim().replace(/[\\/]+$/, "") ?? "";
  if (!dir) return file;
  const separator = dir.includes("\\") ? "\\" : "/";
  return `${dir}${separator}${file}`;
}

export function buildExportRequest(input: {
  sequence: Sequence;
  preset: ExportPresetId;
  range: ExportRange;
  outputPath: string;
}): ExportRequest {
  const outputPath = withPresetExtension(input.outputPath, input.preset);
  if (!outputPath) throw new ExportValidationError("Choose an output path");
  return {
    sequenceId: input.sequence.id,
    preset: input.preset,
    range: resolveExportRange(input.sequence, input.range),
    outputPath,
  };
}

export function validateExportForm(
  sequences: readonly Sequence[],
  form: ExportFormState,
  marks: ExportMarks,
): ExportFormResult {
  const sequence = sequences.find((item) => item.id === form.sequenceId);
  if (!sequence) return { ok: false, message: "Open a sequence to export." };
  try {
    return {
      ok: true,
      request: buildExportRequest({
        sequence,
        preset: form.preset,
        range: {
          mode: form.rangeMode,
          inTicks: marks.inTicks ?? undefined,
          outTicks: marks.outTicks ?? undefined,
        },
        outputPath: form.outputPath,
      }),
    };
  } catch (error) {
    if (error instanceof ExportValidationError) return { ok: false, message: error.message };
    throw error;
  }
}

export async function submitExport(
  exporter: MediaExporter | undefined,
  result: ExportFormResult,
): Promise<{ ok: true } | { ok: false; message: string }> {
  if (!exporter) return { ok: false, message: "Media export is not connected." };
  if (!result.ok) return result;
  try {
    await exporter.exportSequence(result.request);
    return { ok: true };
  } catch (error) {
    const message = error instanceof Error && error.message ? error.message : "Export failed.";
    return { ok: false, message };
  }
}
