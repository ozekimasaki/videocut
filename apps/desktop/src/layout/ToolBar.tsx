import { RedoIcon, ToolIcon, UndoIcon } from "../ui/icons";
import { formatTimecode } from "../model/timecode";
import type { Editor } from "../host/use-editor";
import { TOOLS, toolSpec } from "../host/types";
import { sequenceById } from "../host/playhead";

export function ToolBar({ editor }: { editor: Editor }) {
  const sequence = sequenceById(editor.project, editor.session.activeSequenceId);
  const fps = sequence?.settings.fps ?? { num: 30, den: 1 };
  const timecode = formatTimecode(editor.session.focusedMonitor === "source" ? editor.session.sourcePlayheadTicks : editor.session.playheadTicks, fps);
  return (
    <div className="toolbar" role="toolbar" aria-label="ツール" data-testid="toolbar">
      <div className="toolbar__group">
        {TOOLS.map((tool) => (
          <button
            key={tool.id}
            type="button"
            className={editor.session.tool === tool.id ? "tool is-active" : "tool"}
            aria-pressed={editor.session.tool === tool.id}
            aria-label={tool.label}
            title={`${tool.label} (${tool.shortcut})`}
            data-testid={`tool-${tool.id}`}
            onClick={() => editor.setTool(tool.id)}
          >
            <ToolIcon tool={tool.id} />
          </button>
        ))}
      </div>
      <div className="toolbar__group">
        <button type="button" className="tool" aria-label="取り消し" title="取り消し" disabled={!editor.canUndo} onClick={editor.undo}>
          <UndoIcon />
        </button>
        <button type="button" className="tool" aria-label="やり直し" title="やり直し" disabled={!editor.canRedo} onClick={editor.redo}>
          <RedoIcon />
        </button>
      </div>
      <div className="toolbar__status">
        <span>{toolSpec(editor.session.tool).label}</span>
        <span className="timecode" data-testid="toolbar-timecode">
          {timecode}
        </span>
      </div>
    </div>
  );
}
