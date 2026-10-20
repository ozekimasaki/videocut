import { browseOutputPath, exportMedia } from "../host/desktop-bridge";
import type { Editor } from "../host/use-editor";
import { ExportPanel, type MediaExporter } from "../ui/modules";

export function ExportDialog({ editor, onClose }: { editor: Editor; onClose: () => void }) {
  const exporter: MediaExporter = {
    async exportSequence(request) {
      await exportMedia(editor.project, request);
    },
  };
  return (
    <div className="modal" role="presentation" onClick={onClose}>
      <div className="modal__card" role="dialog" aria-label="書き出し" onClick={(event) => event.stopPropagation()}>
        <ExportPanel
          sequences={editor.project.sequences}
          sequenceId={editor.session.activeSequenceId ?? undefined}
          onSequenceIdChange={(sequenceId) => editor.activateSequence(sequenceId)}
          inTicks={editor.session.programMarkIn}
          outTicks={editor.session.programMarkOut}
          exporter={exporter}
          onBrowseOutput={() => browseOutputPath(editor.project.name)}
        />
        <button type="button" className="modal__close" onClick={onClose}>
          閉じる
        </button>
      </div>
    </div>
  );
}
