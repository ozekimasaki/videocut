import { useEffect, useId, useState } from "react";
import type { Sequence } from "@videocut/model";
import {
  EXPORT_PRESETS,
  formatTimecode,
  isExportPresetId,
  sequenceEndTicks,
  submitExport,
  suggestOutputPath,
  validateExportForm,
  withPresetExtension,
} from "./export";
import type { ExportPresetId, ExportRangeMode, MediaExporter } from "./export";
import "./ExportPanel.css";

export interface ExportPanelProps {
  sequences: readonly Sequence[];
  sequenceId?: string;
  onSequenceIdChange?: (sequenceId: string) => void;
  inTicks?: number | null;
  outTicks?: number | null;
  exporter?: MediaExporter;
  onBrowseOutput?: () => Promise<string | null> | string | null;
  outputDirectory?: string;
  className?: string;
}

export function ExportPanel({
  sequences,
  sequenceId,
  onSequenceIdChange,
  inTicks = null,
  outTicks = null,
  exporter,
  onBrowseOutput,
  outputDirectory,
  className,
}: ExportPanelProps) {
  const controlled = onSequenceIdChange !== undefined;
  const [internalSequenceId, setInternalSequenceId] = useState(
    () => sequenceId ?? sequences[0]?.id ?? "",
  );
  const activeSequenceId = controlled
    ? (sequenceId ?? sequences[0]?.id ?? "")
    : internalSequenceId;
  const [preset, setPreset] = useState<ExportPresetId>("h264");
  const [rangeMode, setRangeMode] = useState<ExportRangeMode>("entire");
  const [outputPath, setOutputPath] = useState(() => {
    const initial = sequences.find((item) => item.id === (sequenceId ?? sequences[0]?.id));
    return initial ? suggestOutputPath(initial.name, "h264", outputDirectory) : "";
  });
  const [pathTouched, setPathTouched] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [banner, setBanner] = useState<{ tone: "ok" | "error"; text: string } | null>(null);
  const rangeName = useId();
  const sequenceFieldId = useId();
  const presetFieldId = useId();
  const outputFieldId = useId();

  const sequence = sequences.find((item) => item.id === activeSequenceId) ?? null;
  const sequenceName = sequence?.name;

  useEffect(() => {
    if (controlled) return;
    if (internalSequenceId && sequences.some((item) => item.id === internalSequenceId)) return;
    setInternalSequenceId(sequences[0]?.id ?? "");
  }, [controlled, internalSequenceId, sequences]);

  useEffect(() => {
    if (pathTouched || !sequenceName) return;
    setOutputPath(suggestOutputPath(sequenceName, preset, outputDirectory));
  }, [pathTouched, sequenceName, preset, outputDirectory]);

  const formResult = validateExportForm(
    sequences,
    { sequenceId: activeSequenceId, preset, rangeMode, outputPath },
    { inTicks, outTicks },
  );
  const disconnected = formResult.ok && !exporter && !banner;
  const hint =
    banner?.text ??
    (!formResult.ok ? formResult.message : disconnected ? "Media export is not connected." : null);
  const tone = banner?.tone ?? (disconnected ? "muted" : "error");

  function selectSequence(id: string) {
    if (onSequenceIdChange) onSequenceIdChange(id);
    else setInternalSequenceId(id);
    setBanner(null);
  }

  function selectPreset(value: string) {
    if (!isExportPresetId(value)) return;
    setPreset(value);
    setBanner(null);
    if (pathTouched) setOutputPath((current) => withPresetExtension(current, value));
  }

  async function handleExport() {
    setExporting(true);
    const outcome = await submitExport(exporter, formResult);
    setExporting(false);
    setBanner(
      outcome.ok
        ? { tone: "ok", text: "Export finished." }
        : { tone: "error", text: outcome.message },
    );
  }

  async function browse() {
    if (!onBrowseOutput) return;
    const picked = await onBrowseOutput();
    if (!picked) return;
    setPathTouched(true);
    setOutputPath(withPresetExtension(picked, preset));
    setBanner(null);
  }

  const marks = sequence ? describeRange(sequence, rangeMode, inTicks, outTicks) : null;

  return (
    <form
      aria-label="Export"
      className={className ? `vc-export ${className}` : "vc-export"}
      onSubmit={(event) => {
        event.preventDefault();
        if (!exporting && formResult.ok && exporter) void handleExport();
      }}
    >
      <label className="vc-export__field" htmlFor={sequenceFieldId}>
        <span className="vc-export__label">Sequence</span>
        <select
          id={sequenceFieldId}
          value={activeSequenceId}
          disabled={sequences.length === 0}
          onChange={(event) => selectSequence(event.target.value)}
        >
          {sequences.length === 0 ? <option value="">No sequence</option> : null}
          {sequences.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
      </label>
      <fieldset className="vc-export__range">
        <legend>Range</legend>
        <label>
          <input
            type="radio"
            name={rangeName}
            value="entire"
            checked={rangeMode === "entire"}
            onChange={() => {
              setRangeMode("entire");
              setBanner(null);
            }}
          />
          Entire sequence
        </label>
        <label>
          <input
            type="radio"
            name={rangeName}
            value="inOut"
            checked={rangeMode === "inOut"}
            onChange={() => {
              setRangeMode("inOut");
              setBanner(null);
            }}
          />
          In/Out
        </label>
      </fieldset>
      {marks ? <p className="vc-export__marks">{marks}</p> : null}
      <label className="vc-export__field" htmlFor={presetFieldId}>
        <span className="vc-export__label">Preset</span>
        <select
          id={presetFieldId}
          value={preset}
          onChange={(event) => selectPreset(event.target.value)}
        >
          {EXPORT_PRESETS.map((item) => (
            <option key={item.id} value={item.id}>
              {item.label}
            </option>
          ))}
        </select>
      </label>
      <div className="vc-export__field">
        <label className="vc-export__label" htmlFor={outputFieldId}>
          Output
        </label>
        <div className="vc-export__path">
          <input
            id={outputFieldId}
            type="text"
            value={outputPath}
            autoComplete="off"
            spellCheck={false}
            aria-label="Output path"
            onChange={(event) => {
              setPathTouched(true);
              setOutputPath(event.target.value);
              setBanner(null);
            }}
          />
          {onBrowseOutput ? (
            <button type="button" onClick={() => void browse()}>
              Browse
            </button>
          ) : null}
        </div>
      </div>
      {hint ? (
        <p className={`vc-export__message vc-export__message--${tone}`} role="status">
          {hint}
        </p>
      ) : null}
      <button
        className="vc-export__submit"
        type="submit"
        disabled={exporting || !formResult.ok || !exporter}
      >
        {exporting ? "Exporting…" : "Export"}
      </button>
    </form>
  );
}

function describeRange(
  sequence: Sequence,
  mode: ExportRangeMode,
  inTicks: number | null,
  outTicks: number | null,
): string {
  const fps = sequence.settings.fps;
  const format = (ticks: number | null) => (ticks == null ? "—" : formatTimecode(ticks, fps));
  switch (mode) {
    case "entire":
      return `00:00:00:00 – ${formatTimecode(sequenceEndTicks(sequence), fps)}`;
    case "inOut":
      return `${format(inTicks)} – ${format(outTicks)}`;
    default: {
      const unknown: never = mode;
      return unknown;
    }
  }
}
