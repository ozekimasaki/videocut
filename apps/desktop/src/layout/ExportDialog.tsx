import { useState } from "react";
import { exportStatus } from "../host/desktop-bridge";
import { EXPORT_EXPORTS, resolvePanel, type PanelComponent } from "../host/panels";
import type { ExportRequest, SequenceExporter } from "../host/panel-props";
import type { Editor } from "../host/use-editor";
import { exportModule } from "../ui/modules";
import { sequenceById } from "../host/playhead";

export function ExportDialog({ editor, onClose }: { editor: Editor; onClose: () => void }) {
  const sequence = sequenceById(editor.project, editor.session.activeSequenceId);
  const exporter: SequenceExporter = {
    async exportSequence(request) {
      const message = await exportStatus();
      return { ok: false, message: `${message}（${request.preset}）` };
    },
  };
  const External = resolvePanel(exportModule, EXPORT_EXPORTS);
  return (
    <div className="modal" role="presentation" onClick={onClose}>
      <div className="modal__card" role="dialog" aria-label="書き出し" onClick={(event) => event.stopPropagation()}>
        {External ? (
          <ExportHost
            panel={External}
            editor={editor}
            exporter={exporter}
          />
        ) : (
          <FallbackExport
            sequenceName={sequence?.name ?? "シーケンスなし"}
            canInOut={editor.session.programMarkIn != null && editor.session.programMarkOut != null}
            exporter={exporter}
            sequenceId={editor.session.activeSequenceId}
          />
        )}
        <button type="button" className="modal__close" onClick={onClose}>
          閉じる
        </button>
      </div>
    </div>
  );
}

function ExportHost({
  panel: Panel,
  editor,
  exporter,
}: {
  panel: PanelComponent;
  editor: Editor;
  exporter: SequenceExporter;
}) {
  const sequence = sequenceById(editor.project, editor.session.activeSequenceId);
  const props = {
    project: editor.project,
    sequence,
    sequenceId: editor.session.activeSequenceId,
    tool: editor.session.tool,
    playheadTicks: editor.session.playheadTicks,
    selectedClipIds: editor.session.selectedClipIds,
    markInTicks: editor.session.programMarkIn,
    markOutTicks: editor.session.programMarkOut,
    execute: editor.execute,
    exporter,
  };
  return <Panel {...props} />;
}

function FallbackExport({
  sequenceName,
  canInOut,
  exporter,
  sequenceId,
}: {
  sequenceName: string;
  canInOut: boolean;
  exporter: SequenceExporter;
  sequenceId: string | null;
}) {
  const [range, setRange] = useState<ExportRequest["range"]>("entire");
  const [preset, setPreset] = useState<ExportRequest["preset"]>("h264");
  const [outputPath, setOutputPath] = useState("output.mp4");
  const [message, setMessage] = useState("");
  return (
    <form
      className="export"
      data-testid="export-fallback"
      onSubmit={(event) => {
        event.preventDefault();
        if (!sequenceId) return;
        void exporter.exportSequence({ sequenceId, range, preset, outputPath }).then((result) => setMessage(result.message));
      }}
    >
      <h2>書き出し</h2>
      <p>{sequenceName}</p>
      <label>
        範囲
        <select value={range} onChange={(event) => setRange(event.target.value as ExportRequest["range"])}>
          <option value="entire">シーケンス全体</option>
          <option value="in-out" disabled={!canInOut}>
            インからアウト
          </option>
        </select>
      </label>
      <label>
        プリセット
        <select value={preset} onChange={(event) => setPreset(event.target.value as ExportRequest["preset"])}>
          <option value="h264">H.264</option>
          <option value="prores">ProRes</option>
          <option value="wav">WAV</option>
        </select>
      </label>
      <label>
        出力
        <input value={outputPath} onChange={(event) => setOutputPath(event.target.value)} />
      </label>
      <button type="submit">書き出す</button>
      {message ? <p className="hint">{message}</p> : null}
    </form>
  );
}
