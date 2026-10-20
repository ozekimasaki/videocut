import type { Editor } from "../host/use-editor";
import { projectCommand } from "../host/document";
import { fromTimelineTool, toTimelineTool } from "../host/timeline-tool";
import { Timeline as TimelineView } from "../ui/modules";

export function Timeline({ editor }: { editor: Editor }) {
  const sequence = editor.project.sequences.find((item) => item.id === editor.session.activeSequenceId) ?? null;
  if (!sequence) {
    return (
      <section className="panel timeline" data-testid="timeline">
        <p className="empty">シーケンスがありません</p>
      </section>
    );
  }
  return (
    <section className="panel timeline">
      <TimelineView
        sequence={sequence}
        playheadTicks={editor.session.playheadTicks}
        tool={toTimelineTool(editor.session.tool)}
        selectedClipIds={editor.session.selectedClipIds}
        onSequenceChange={(next) => {
          editor.execute(projectCommand("タイムライン", (project) => ({
            ...project,
            sequences: project.sequences.map((item) => (item.id === next.id ? next : item)),
          })));
        }}
        onPlayheadChange={editor.setPlayhead}
        onSelectionChange={editor.setSelected}
        onToolChange={(tool) => editor.setTool(fromTimelineTool(tool))}
      />
    </section>
  );
}
