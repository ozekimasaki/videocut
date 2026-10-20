import { useRef, useState } from "react";
import type { Clip, Sequence, Track } from "@videocut/model";
import { TICKS_PER_SECOND } from "@videocut/model";
import { commands } from "../host/edits";
import { TIMELINE_EXPORTS, resolvePanel } from "../host/panels";
import type { PanelHostProps } from "../host/panel-props";
import { sequenceEnd, snapTicks } from "../host/playhead";
import type { ToolId } from "../host/types";
import type { Editor } from "../host/use-editor";
import { pixelsToTicks, ticksToPixels } from "../model/timecode";
import { labelColor } from "../ui/colors";
import { timelineModule } from "../ui/modules";

const HEADER = 132;

export function Timeline({ editor }: { editor: Editor }) {
  const sequence = editor.project.sequences.find((item) => item.id === editor.session.activeSequenceId) ?? null;
  const host: PanelHostProps = {
    project: editor.project,
    sequence,
    sequenceId: editor.session.activeSequenceId,
    tool: editor.session.tool,
    playheadTicks: editor.session.playheadTicks,
    selectedClipIds: editor.session.selectedClipIds,
    markInTicks: editor.session.programMarkIn,
    markOutTicks: editor.session.programMarkOut,
    execute: editor.execute,
    setPlayhead: editor.setPlayhead,
    setSelectedClipIds: editor.setSelected,
  };
  const External = resolvePanel(timelineModule, TIMELINE_EXPORTS);
  if (External) return <External {...host} />;
  return <FallbackTimeline editor={editor} sequence={sequence} />;
}

function FallbackTimeline({ editor, sequence }: { editor: Editor; sequence: Sequence | null }) {
  const [pxPerSecond, setPxPerSecond] = useState(72);
  const scrollerRef = useRef<HTMLDivElement>(null);
  const gesture = useRef<Gesture | null>(null);
  if (!sequence || !editor.session.activeSequenceId) {
    return (
      <section className="panel timeline" data-testid="timeline">
        <p className="empty">シーケンスがありません</p>
      </section>
    );
  }
  const sequenceId = editor.session.activeSequenceId;
  const duration = Math.max(sequenceEnd(sequence) + TICKS_PER_SECOND * 4, TICKS_PER_SECOND * 20);
  const laneWidth = Math.max(480, ticksToPixels(duration, pxPerSecond));
  const tracks = [...sequence.videoTracks].sort((a, b) => b.index - a.index).concat(sequence.audioTracks);
  const playheadX = HEADER + ticksToPixels(editor.session.playheadTicks, pxPerSecond);

  const ticksAt = (clientX: number) => {
    const scroller = scrollerRef.current;
    if (!scroller) return 0;
    const rect = scroller.getBoundingClientRect();
    const x = clientX - rect.left + scroller.scrollLeft - HEADER;
    return Math.max(0, pixelsToTicks(x, pxPerSecond));
  };
  const snapped = (ticks: number) => {
    const points = [0, editor.session.playheadTicks];
    if (editor.session.programMarkIn != null) points.push(editor.session.programMarkIn);
    if (editor.session.programMarkOut != null) points.push(editor.session.programMarkOut);
    for (const track of tracks) {
      for (const clip of track.clips) points.push(clip.timelineIn, clip.timelineOut);
    }
    for (const marker of sequence.markers ?? []) points.push(marker.timeTicks);
    return snapTicks(ticks, points, pixelsToTicks(8, pxPerSecond));
  };

  return (
    <section className="panel timeline" data-testid="timeline">
      <header className="panel__title">
        <span>{sequence.name}</span>
        <div className="panel__actions">
          <button type="button" onClick={() => setPxPerSecond((value) => Math.max(24, value / 1.25))}>
            −
          </button>
          <button type="button" onClick={() => setPxPerSecond((value) => Math.min(280, value * 1.25))}>
            ＋
          </button>
        </div>
      </header>
      <div
        className="timeline__scroll"
        ref={scrollerRef}
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          const target = event.target as HTMLElement;
          if (target.closest(".track__head")) return;
          const raw = ticksAt(event.clientX);
          const time = snapped(raw);
          const clipEl = target.closest("[data-clip-id]") as HTMLElement | null;
          if (!clipEl) {
            if (editor.session.tool === "pen") {
              editor.execute(commands.marker(sequenceId, time));
              return;
            }
            editor.setPlayhead(time);
            editor.setSelected([]);
            gesture.current = { kind: "seek" };
            event.currentTarget.setPointerCapture(event.pointerId);
            return;
          }
          const clipId = clipEl.dataset.clipId ?? "";
          const edge = edgeAt(clipEl, event.clientX);
          const tool = editor.session.tool;
          if (tool === "razor") {
            editor.execute(commands.razor(sequenceId, time));
            editor.setPlayhead(time);
            return;
          }
          if (tool === "pen") {
            editor.execute(commands.marker(sequenceId, time));
            return;
          }
          if (tool === "trackSelectForward" || tool === "trackSelectBackward") {
            editor.setSelected(trackSelection(sequence, clipId, tool));
            return;
          }
          const selected = event.shiftKey
            ? toggleId(editor.session.selectedClipIds, clipId)
            : editor.session.selectedClipIds.includes(clipId)
              ? editor.session.selectedClipIds
              : [clipId];
          editor.setSelected(selected);
          const origin = edge === "body" ? raw : edgeTime(sequence, clipId, edge);
          gesture.current = { kind: gestureKind(tool, edge), clipId, start: raw, origin, edge, moved: false };
          event.currentTarget.setPointerCapture(event.pointerId);
        }}
        onPointerMove={(event) => {
          const current = gesture.current;
          if (!current) return;
          const time = snapped(ticksAt(event.clientX));
          if (current.kind === "seek") {
            editor.setPlayhead(time);
            return;
          }
          const delta = time - current.start;
          if (Math.abs(delta) < pixelsToTicks(2, pxPerSecond)) return;
          current.moved = true;
          const command = commandFor(current, sequenceId, time, delta);
          if (command) editor.previewCommand(command);
        }}
        onPointerUp={(event) => {
          const current = gesture.current;
          gesture.current = null;
          if (!current || current.kind === "seek") {
            editor.previewCommand(null);
            return;
          }
          if (!current.moved) {
            editor.previewCommand(null);
            return;
          }
          const time = snapped(ticksAt(event.clientX));
          const command = commandFor(current, sequenceId, time, time - current.start);
          editor.previewCommand(null);
          if (command) editor.execute(command);
        }}
      >
        <div className="timeline__content" style={{ width: HEADER + laneWidth }}>
          <div className="ruler">
            <div className="ruler__corner" />
            <div className="ruler__lane" style={{ width: laneWidth }}>
              {Array.from({ length: Math.ceil(duration / TICKS_PER_SECOND) + 1 }, (_, second) => (
                <span key={second} className="ruler__label" style={{ left: ticksToPixels(second * TICKS_PER_SECOND, pxPerSecond) }}>
                  {second}s
                </span>
              ))}
              {(sequence.markers ?? []).map((marker) => (
                <i key={marker.id} className="marker" style={{ left: ticksToPixels(marker.timeTicks, pxPerSecond) }} title={marker.name} />
              ))}
            </div>
          </div>
          <div className="lanes">
            {tracks.map((track) => (
              <div className={track.kind === "video" ? "track track--video" : "track track--audio"} key={track.id}>
                <div className="track__head">
                  <strong>{track.kind === "video" ? `V${track.index + 1}` : `A${track.index + 1}`}</strong>
                  <div className="track__toggles">
                    <TrackToggle label="対象" pressed={track.targeted} onClick={() => editor.execute(commands.patchTrack(sequenceId, track.id, { targeted: !track.targeted }))} />
                    <TrackToggle label="ロック" pressed={track.locked} onClick={() => editor.execute(commands.patchTrack(sequenceId, track.id, { locked: !track.locked }))} />
                    <TrackToggle label="ミュート" pressed={track.muted} onClick={() => editor.execute(commands.patchTrack(sequenceId, track.id, { muted: !track.muted }))} />
                    <TrackToggle label="同期" pressed={track.syncLock} onClick={() => editor.execute(commands.patchTrack(sequenceId, track.id, { syncLock: !track.syncLock }))} />
                    <TrackToggle label="非表示" pressed={track.hidden} onClick={() => editor.execute(commands.patchTrack(sequenceId, track.id, { hidden: !track.hidden }))} />
                  </div>
                </div>
                <div className="track__lane" style={{ width: laneWidth }}>
                  {track.clips.map((clip) => (
                    <div
                      key={clip.id}
                      data-clip-id={clip.id}
                      className={editor.session.selectedClipIds.includes(clip.id) ? "clip is-selected" : "clip"}
                      style={{
                        left: ticksToPixels(clip.timelineIn, pxPerSecond),
                        width: Math.max(4, ticksToPixels(clip.timelineOut - clip.timelineIn, pxPerSecond)),
                        background: labelColor(clip.labelColor),
                      }}
                      onDoubleClick={(event) => {
                        event.stopPropagation();
                        editor.openSourceClip(clip.id);
                      }}
                    >
                      <span>{clip.name}</span>
                    </div>
                  ))}
                </div>
              </div>
            ))}
            <div className="playhead" style={{ left: playheadX }} data-testid="playhead" />
          </div>
        </div>
      </div>
    </section>
  );
}

function TrackToggle({ label, pressed, onClick }: { label: string; pressed: boolean; onClick: () => void }) {
  return (
    <button type="button" className={pressed ? "toggle is-on" : "toggle"} aria-pressed={pressed} title={label} onClick={onClick}>
      {label.slice(0, 1)}
    </button>
  );
}

type GestureKind = "seek" | "move" | "trim" | "ripple" | "roll" | "slip" | "slide";

type Gesture =
  | { kind: "seek" }
  | {
      kind: Exclude<GestureKind, "seek">;
      clipId: string;
      start: number;
      origin: number;
      edge: "start" | "end" | "body";
      moved: boolean;
    };

function gestureKind(tool: ToolId, edge: "start" | "end" | "body"): GestureKind {
  switch (tool) {
    case "select":
      return edge === "body" ? "move" : "trim";
    case "ripple":
      return edge === "body" ? "move" : "ripple";
    case "rolling":
      return edge === "body" ? "move" : "roll";
    case "slip":
      return "slip";
    case "slide":
      return "slide";
    case "razor":
    case "pen":
    case "trackSelectForward":
    case "trackSelectBackward":
      return "move";
    default: {
      const unknown: never = tool;
      throw new Error(String(unknown));
    }
  }
}

function commandFor(gesture: Gesture, sequenceId: string, time: number, delta: number) {
  if (gesture.kind === "seek") return null;
  switch (gesture.kind) {
    case "move":
      return commands.move(sequenceId, [gesture.clipId], delta);
    case "trim":
      return commands.trim(sequenceId, gesture.clipId, gesture.edge === "start" ? "start" : "end", gesture.origin + delta);
    case "ripple":
      return commands.rippleEdge(
        sequenceId,
        gesture.clipId,
        gesture.edge === "start" ? "start" : "end",
        time - gesture.origin,
      );
    case "roll":
      return commands.roll(sequenceId, gesture.clipId, gesture.edge === "start" ? "start" : "end", gesture.origin + delta);
    case "slip":
      return commands.slip(sequenceId, gesture.clipId, delta);
    case "slide":
      return commands.slide(sequenceId, gesture.clipId, delta);
    default: {
      const unknown: never = gesture;
      return unknown;
    }
  }
}

function edgeAt(element: HTMLElement, clientX: number): "start" | "end" | "body" {
  const rect = element.getBoundingClientRect();
  if (clientX - rect.left <= 6) return "start";
  if (rect.right - clientX <= 6) return "end";
  return "body";
}

function edgeTime(sequence: Sequence, clipId: string, edge: "start" | "end" | "body"): number {
  const clip = find(sequence, clipId);
  if (!clip || edge === "body") return clip?.timelineIn ?? 0;
  return edge === "start" ? clip.timelineIn : clip.timelineOut;
}

function find(sequence: Sequence, clipId: string): Clip | null {
  for (const track of [...sequence.videoTracks, ...sequence.audioTracks]) {
    const clip = track.clips.find((item) => item.id === clipId);
    if (clip) return clip;
  }
  return null;
}

function trackSelection(sequence: Sequence, clipId: string, tool: "trackSelectForward" | "trackSelectBackward"): string[] {
  for (const track of [...sequence.videoTracks, ...sequence.audioTracks] as Track[]) {
    const clip = track.clips.find((item) => item.id === clipId);
    if (!clip) continue;
    return track.clips
      .filter((item) => (tool === "trackSelectForward" ? item.timelineIn >= clip.timelineIn : item.timelineIn <= clip.timelineIn))
      .map((item) => item.id);
  }
  return [clipId];
}

function toggleId(ids: string[], id: string): string[] {
  return ids.includes(id) ? ids.filter((item) => item !== id) : [...ids, id];
}
