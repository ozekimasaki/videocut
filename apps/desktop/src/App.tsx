import { useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import { programPicture, sequenceById, sourcePicture } from "./host/playhead";
import { useEditor } from "./host/use-editor";
import { ExportDialog } from "./layout/ExportDialog";
import { MenuBar } from "./layout/MenuBar";
import { Monitor } from "./layout/Monitor";
import { ProjectBin } from "./layout/ProjectBin";
import { SidePanels } from "./layout/SidePanels";
import { Timeline } from "./layout/Timeline";
import { ToolBar } from "./layout/ToolBar";

export function App() {
  const editor = useEditor();
  const [exportOpen, setExportOpen] = useState(false);
  const [binWidth, setBinWidth] = useState(280);
  const [sideWidth, setSideWidth] = useState(320);
  const [monitorHeight, setMonitorHeight] = useState(320);
  const sequence = sequenceById(editor.project, editor.session.activeSequenceId);
  const fps = sequence?.settings.fps ?? { num: 30, den: 1 };
  const program = programPicture(editor.project, sequence, editor.session.playheadTicks);
  const source = sourcePicture(
    editor.project,
    sequence,
    editor.session.sourceMediaId,
    editor.session.sourceClipId,
    editor.session.sourcePlayheadTicks,
  );
  return (
    <div className="shell" data-testid="shell">
      <MenuBar editor={editor} onExport={() => setExportOpen(true)} />
      <ToolBar editor={editor} />
      <div
        className="workspace"
        style={{
          gridTemplateColumns: `${binWidth}px 6px minmax(0, 1fr) 6px ${sideWidth}px`,
          gridTemplateRows: `${monitorHeight}px 6px minmax(160px, 1fr)`,
        }}
      >
        <div className="area-bin">
          <ProjectBin editor={editor} />
        </div>
        <Splitter axis="x" className="area-vsplit-bin" onDelta={(delta) => setBinWidth((current) => clamp(current + delta, 200, 460))} />
        <div className="area-monitors">
          <Monitor
            picture={source}
            fps={fps}
            focused={editor.session.focusedMonitor === "source"}
            playing={editor.session.playing?.monitor === "source"}
            testId="source-monitor"
            onFocus={() => editor.focusMonitor("source")}
            onTogglePlay={() => editor.togglePlay("source")}
            onStep={(direction) => editor.step("source", direction)}
            extra={
              <button type="button" onClick={editor.overwriteSource}>
                上書き
              </button>
            }
          />
          <Monitor
            picture={program}
            fps={fps}
            focused={editor.session.focusedMonitor === "program"}
            playing={editor.session.playing?.monitor === "program"}
            testId="program-monitor"
            onFocus={() => editor.focusMonitor("program")}
            onTogglePlay={() => editor.togglePlay("program")}
            onStep={(direction) => editor.step("program", direction)}
          />
        </div>
        <Splitter axis="y" className="area-hsplit" onDelta={(delta) => setMonitorHeight((current) => clamp(current + delta, 180, 560))} />
        <div className="area-timeline">
          <Timeline editor={editor} />
        </div>
        <Splitter axis="x" className="area-vsplit-side" onDelta={(delta) => setSideWidth((current) => clamp(current - delta, 240, 480))} />
        <div className="area-side">
          <SidePanels editor={editor} />
        </div>
      </div>
      <footer className="status" data-testid="status">
        <span>{editor.session.message}</span>
        <span>{editor.dirty ? "未保存" : "保存済み"}</span>
        <span>
          {sequence ? `${sequence.settings.width}×${sequence.settings.height}` : ""}
        </span>
      </footer>
      {exportOpen ? <ExportDialog editor={editor} onClose={() => setExportOpen(false)} /> : null}
    </div>
  );
}

function Splitter({ axis, className, onDelta }: { axis: "x" | "y"; className: string; onDelta: (delta: number) => void }) {
  const startDrag = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    const origin = axis === "x" ? event.clientX : event.clientY;
    let last = origin;
    const move = (next: PointerEvent) => {
      const point = axis === "x" ? next.clientX : next.clientY;
      onDelta(point - last);
      last = point;
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };
  return (
    <div
      className={axis === "x" ? `splitter splitter--x ${className}` : `splitter splitter--y ${className}`}
      role="separator"
      aria-orientation={axis === "x" ? "vertical" : "horizontal"}
      onPointerDown={startDrag}
    />
  );
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
