import { useEffect, useRef, useState, type PointerEvent, type ReactElement } from "react";
import { TICKS_PER_SECOND, type Clip, type Sequence, type Track } from "@videocut/model";
import {
  EditError,
  extract,
  freezeFrame,
  lift,
  linkClips,
  nest,
  razor,
  rippleDelete,
  rippleTrim,
  rollTrim,
  setDuration,
  setReversed,
  setSpeed,
  setSyncLock,
  setTrackLocked,
  setTrackTargeted,
  slide,
  slip,
  snapTicks,
  switchMulticamAngle,
  unlinkClip,
  addMarker,
  type TrimSide,
} from "./edits";
export type TimelineTool = "select" | "razor" | "ripple" | "roll" | "slip" | "slide";

const TOOLS: Array<{ id: TimelineTool; label: string }> = [
  { id: "select", label: "選択" },
  { id: "razor", label: "レーザー" },
  { id: "ripple", label: "リップル" },
  { id: "roll", label: "ロール" },
  { id: "slip", label: "スリップ" },
  { id: "slide", label: "スライド" },
];

export interface TimelineProps {
  sequence: Sequence;
  playheadTicks?: number;
  tool?: TimelineTool;
  pixelsPerSecond?: number;
  snap?: boolean;
  snapThresholdTicks?: number;
  selectedClipIds?: string[];
  showToolbar?: boolean;
  onSequenceChange?: (sequence: Sequence) => void;
  onPlayheadChange?: (ticks: number) => void;
  onSelectionChange?: (clipIds: string[]) => void;
  onToolChange?: (tool: TimelineTool) => void;
}

interface DragState {
  kind: "ripple" | "roll" | "slip" | "slide";
  clipId: string;
  side: TrimSide;
  origin: number;
  current: number;
}

function cursorFor(tool: TimelineTool): string {
  switch (tool) {
    case "select":
      return "default";
    case "razor":
      return "crosshair";
    case "ripple":
    case "roll":
      return "ew-resize";
    case "slip":
    case "slide":
      return "grab";
    default: {
      const unexpected: never = tool;
      return unexpected;
    }
  }
}

function rowsOf(sequence: Sequence): Array<{ track: Track; audio: boolean }> {
  return [
    ...sequence.videoTracks.map((track) => ({ track, audio: false })),
    ...sequence.audioTracks.map((track) => ({ track, audio: true })),
  ];
}

function clipKind(clip: Clip, audio: boolean): string {
  if (clip.speed === 0) {
    return "freeze";
  }
  if (clip.kind === "nested") {
    return "nested";
  }
  return audio ? "audio" : clip.kind;
}

export function Timeline({
  sequence,
  playheadTicks,
  tool,
  pixelsPerSecond = 80,
  snap = true,
  snapThresholdTicks = TICKS_PER_SECOND / 10,
  selectedClipIds,
  showToolbar = true,
  onSequenceChange,
  onPlayheadChange,
  onSelectionChange,
  onToolChange,
}: TimelineProps): ReactElement {
  const controlledSequence = onSequenceChange !== undefined;
  const [localSequence, setLocalSequence] = useState(sequence);
  const [localPlayhead, setLocalPlayhead] = useState(playheadTicks ?? 0);
  const [localTool, setLocalTool] = useState<TimelineTool>(tool ?? "select");
  const [localSelection, setLocalSelection] = useState<string[]>(selectedClipIds ?? []);
  const [markIn, setMarkIn] = useState<number | undefined>();
  const [markOut, setMarkOut] = useState<number | undefined>();
  const [speed, setSpeedValue] = useState(2);
  const [durationSeconds, setDurationSeconds] = useState(2);
  const [holdSeconds, setHoldSeconds] = useState(1);
  const [angleId, setAngleId] = useState("");
  const [snapOn, setSnapOn] = useState(snap);
  const [error, setError] = useState("");
  const [drag, setDrag] = useState<DragState | undefined>();
  const dragRef = useRef<DragState | undefined>(undefined);
  const laneRef = useRef<HTMLDivElement>(null);
  dragRef.current = drag;

  useEffect(() => {
    if (controlledSequence) {
      setLocalSequence(sequence);
    }
  }, [controlledSequence, sequence]);

  const model = controlledSequence ? sequence : localSequence;
  const playhead = playheadTicks ?? localPlayhead;
  const activeTool = tool ?? localTool;
  const selection = selectedClipIds ?? localSelection;
  const secondsWide = Math.max(8, endOf(model) / TICKS_PER_SECOND + 2);
  const laneWidth = secondsWide * pixelsPerSecond;

  function emitSequence(next: Sequence): void {
    if (controlledSequence) {
      onSequenceChange?.(next);
    } else {
      setLocalSequence(next);
    }
  }

  function emitPlayhead(ticks: number): void {
    const next = Math.max(0, ticks);
    onPlayheadChange?.(next);
    if (playheadTicks === undefined) {
      setLocalPlayhead(next);
    }
  }

  function emitSelection(ids: string[]): void {
    onSelectionChange?.(ids);
    if (selectedClipIds === undefined) {
      setLocalSelection(ids);
    }
  }

  function emitTool(next: TimelineTool): void {
    onToolChange?.(next);
    if (tool === undefined) {
      setLocalTool(next);
    }
  }

  function apply(edit: (draft: Sequence) => void): void {
    const draft = structuredClone(model);
    try {
      edit(draft);
      setError("");
      emitSequence(draft);
    } catch (caught) {
      setError(caught instanceof EditError ? caught.message : "編集できません");
    }
  }

  function snapped(ticks: number, ignore: string[] = []): number {
    if (!snapOn) {
      return Math.max(0, Math.round(ticks));
    }
    return snapTicks(model, ticks, snapThresholdTicks, playhead, ignore);
  }

  function ticksFromClient(clientX: number): number {
    const origin = laneRef.current?.getBoundingClientRect().left ?? 0;
    return ((clientX - origin) / pixelsPerSecond) * TICKS_PER_SECOND;
  }

  function xOf(ticks: number): number {
    return (ticks / TICKS_PER_SECOND) * pixelsPerSecond;
  }

  function chooseTool(next: TimelineTool): void {
    emitTool(next);
  }

  function onKeyDown(key: string): void {
    switch (key.toLowerCase()) {
      case "v":
        chooseTool("select");
        break;
      case "c":
        chooseTool("razor");
        break;
      case "b":
        chooseTool("ripple");
        break;
      case "n":
        chooseTool("roll");
        break;
      case "y":
        chooseTool("slip");
        break;
      case "u":
        chooseTool("slide");
        break;
      default:
        break;
    }
  }

  function finishDrag(state: DragState): void {
    const target = snapped(state.current, [state.clipId]);
    const delta = target - state.origin;
    if (delta === 0) {
      return;
    }
    apply((draft) => {
      switch (state.kind) {
        case "ripple":
          rippleTrim(draft, state.clipId, state.side, state.side === "in" ? -delta : delta);
          break;
        case "roll":
          rollTrim(draft, state.clipId, delta);
          break;
        case "slip": {
          const clip = findClip(draft, state.clipId);
          slip(draft, state.clipId, Math.round(delta * (clip?.speed || 1)));
          break;
        }
        case "slide":
          slide(draft, state.clipId, delta);
          break;
        default: {
          const unexpected: never = state.kind;
          throw new EditError("invalid-time", String(unexpected));
        }
      }
    });
  }

  function onClipPointerDown(event: PointerEvent<HTMLButtonElement>, clip: Clip, track: Track): void {
    event.stopPropagation();
    const at = ticksFromClient(event.clientX);
    const bounds = event.currentTarget.getBoundingClientRect();
    const edge: TrimSide | undefined =
      event.clientX - bounds.left < 8 ? "in" : bounds.right - event.clientX < 8 ? "out" : undefined;
    if (activeTool === "razor") {
      apply((draft) => {
        razor(draft, snapped(at), [track.id]);
      });
      return;
    }
    if (activeTool === "select" || !edge && (activeTool === "ripple" || activeTool === "roll")) {
      emitSelection([clip.id]);
      emitPlayhead(snapped(at));
      return;
    }
    emitSelection([clip.id]);
    if (activeTool === "ripple" && edge) {
      setDrag({
        kind: "ripple",
        clipId: clip.id,
        side: edge,
        origin: edge === "in" ? clip.timelineIn : clip.timelineOut,
        current: at,
      });
      return;
    }
    if (activeTool === "roll" && edge) {
      const left = edge === "out" ? clip : neighbor(track, clip, "prev");
      if (!left) {
        setError("隣り合うクリップがありません");
        return;
      }
      setDrag({
        kind: "roll",
        clipId: left.id,
        side: "out",
        origin: left.timelineOut,
        current: at,
      });
      return;
    }
    if (activeTool === "slip" || activeTool === "slide") {
      setDrag({
        kind: activeTool,
        clipId: clip.id,
        side: "out",
        origin: at,
        current: at,
      });
    }
  }

  useEffect(() => {
    if (!drag) {
      return undefined;
    }
    const move = (event: globalThis.PointerEvent): void => {
      setDrag((current) => current && { ...current, current: ticksFromClient(event.clientX) });
    };
    const up = (): void => {
      const current = dragRef.current;
      dragRef.current = undefined;
      setDrag(undefined);
      if (current) {
        finishDrag(current);
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
  }, [drag?.kind, drag?.clipId]);

  const angles = model.multicamAngles ?? [];

  return (
    <section
      className="vcut-timeline"
      data-testid="timeline"
      aria-label="タイムライン"
      tabIndex={0}
      onKeyDown={(event) => onKeyDown(event.key)}
      style={{ cursor: cursorFor(activeTool) }}
    >
      {showToolbar ? (
        <div className="vcut-timeline__toolbar">
          {TOOLS.map((item) => (
            <button
              key={item.id}
              type="button"
              aria-pressed={activeTool === item.id}
              onClick={() => chooseTool(item.id)}
            >
              {item.label}
            </button>
          ))}
          <button type="button" aria-pressed={snapOn} onClick={() => setSnapOn((value) => !value)}>
            スナップ
          </button>
        </div>
      ) : null}
      <div className="vcut-timeline__commands">
        <button type="button" onClick={() => { setMarkIn(playhead); }}>イン</button>
        <button type="button" onClick={() => { setMarkOut(playhead); }}>アウト</button>
        <button
          type="button"
          onClick={() => applyRange((draft, start, end) => { lift(draft, start, end); })}
        >
          リフト
        </button>
        <button
          type="button"
          onClick={() => applyRange((draft, start, end) => { extract(draft, start, end); })}
        >
          抽出
        </button>
        <button type="button" disabled={selection.length === 0} onClick={() => apply((draft) => rippleDelete(draft, selection))}>
          リップル削除
        </button>
        <button type="button" disabled={selection.length < 2} onClick={() => apply((draft) => linkClips(draft, selection))}>
          リンク
        </button>
        <button type="button" disabled={selection.length === 0} onClick={() => apply((draft) => { for (const id of selection) unlinkClip(draft, id); })}>
          リンク解除
        </button>
        <label>
          速度
          <input
            aria-label="速度"
            type="number"
            min={0.01}
            step={0.1}
            value={speed}
            onChange={(event) => setSpeedValue(Number(event.target.value))}
          />
        </label>
        <button type="button" disabled={selection[0] === undefined} onClick={() => apply((draft) => setSpeed(draft, selection[0]!, speed, true))}>
          適用
        </button>
        <label>
          尺
          <input
            aria-label="尺（秒）"
            type="number"
            min={0.01}
            step={0.1}
            value={durationSeconds}
            onChange={(event) => setDurationSeconds(Number(event.target.value))}
          />
        </label>
        <button
          type="button"
          disabled={selection[0] === undefined}
          onClick={() => apply((draft) => setDuration(draft, selection[0]!, Math.round(durationSeconds * TICKS_PER_SECOND), true))}
        >
          尺を適用
        </button>
        <button type="button" disabled={selection[0] === undefined} onClick={() => apply((draft) => setReversed(draft, selection[0]!, !findClip(model, selection[0]!)?.reversed))}>
          逆再生
        </button>
        <button type="button" disabled={selection.length === 0} onClick={() => apply((draft) => { nest(draft, selection, "ネスト"); })}>
          ネスト
        </button>
        <label>
          フリーズ
          <input
            aria-label="フリーズ秒数"
            type="number"
            min={0.01}
            step={0.1}
            value={holdSeconds}
            onChange={(event) => setHoldSeconds(Number(event.target.value))}
          />
        </label>
        <button
          type="button"
          disabled={selection[0] === undefined}
          onClick={() => apply((draft) => { freezeFrame(draft, selection[0]!, playhead, Math.round(holdSeconds * TICKS_PER_SECOND)); })}
        >
          静止画
        </button>
        <button
          type="button"
          onClick={() => apply((draft) => {
            addMarker(draft, {
              timeTicks: playhead,
              durationTicks: 0,
              name: "マーカー",
              comment: "",
              color: 1,
              kind: "comment",
            });
          })}
        >
          マーカー
        </button>
        <select aria-label="マルチカムアングル" value={angleId} onChange={(event) => setAngleId(event.target.value)}>
          <option value="">アングル</option>
          {angles.map((angle) => (
            <option key={angle.id} value={angle.id}>{angle.name}</option>
          ))}
        </select>
        <button
          type="button"
          disabled={selection[0] === undefined || angleId === ""}
          onClick={() => apply((draft) => { switchMulticamAngle(draft, selection[0]!, playhead, angleId); })}
        >
          アングル切替
        </button>
      </div>
      {error ? <p className="vcut-timeline__error" role="status">{error}</p> : null}
      <div className="vcut-timeline__scroll">
        <div className="vcut-timeline__ruler">
          <div className="vcut-timeline__head">ルーラー</div>
          <div
            ref={laneRef}
            className="vcut-timeline__lane"
            style={{ width: laneWidth }}
            onPointerDown={(event) => emitPlayhead(snapped(ticksFromClient(event.clientX)))}
          >
            {Array.from({ length: Math.ceil(secondsWide) }, (_, second) => (
              <span key={second} className="vcut-timeline__tick" style={{ left: second * pixelsPerSecond }}>
                {second}s
              </span>
            ))}
            {(model.markers ?? []).map((marker) => (
              <span
                key={marker.id}
                className="vcut-timeline__marker"
                title={marker.name}
                style={{ left: xOf(marker.timeTicks) }}
              />
            ))}
            <span className="vcut-timeline__playhead" style={{ left: xOf(playhead) }} />
          </div>
        </div>
        {rowsOf(model).map(({ track: row, audio }) => (
          <div className="vcut-timeline__track" key={row.id} data-testid={`track-${row.id}`}>
            <div className="vcut-timeline__head">
              <span>{audio ? `A${row.index + 1}` : `V${row.index + 1}`}</span>
              <button type="button" aria-pressed={row.targeted} aria-label="ターゲット" onClick={() => apply((draft) => setTrackTargeted(draft, row.id, !row.targeted))}>T</button>
              <button type="button" aria-pressed={row.syncLock} aria-label="シンクロック" onClick={() => apply((draft) => setSyncLock(draft, row.id, !row.syncLock))}>S</button>
              <button type="button" aria-pressed={row.locked} aria-label="ロック" onClick={() => apply((draft) => setTrackLocked(draft, row.id, !row.locked))}>L</button>
            </div>
            <div className="vcut-timeline__lane" style={{ width: laneWidth }} onPointerDown={(event) => emitPlayhead(snapped(ticksFromClient(event.clientX)))}>
              {row.clips.map((clip) => (
                <button
                  key={clip.id}
                  type="button"
                  className="vcut-timeline__clip"
                  data-testid={`clip-${clip.id}`}
                  data-kind={clipKind(clip, audio)}
                  data-selected={selection.includes(clip.id)}
                  style={{ left: xOf(clip.timelineIn), width: Math.max(8, xOf(clip.timelineOut - clip.timelineIn)) }}
                  onPointerDown={(event) => onClipPointerDown(event, clip, row)}
                >
                  {clip.name}
                </button>
              ))}
              <span className="vcut-timeline__playhead" style={{ left: xOf(playhead) }} />
            </div>
          </div>
        ))}
      </div>
    </section>
  );

  function applyRange(edit: (draft: Sequence, start: number, end: number) => void): void {
    if (markIn === undefined || markOut === undefined || markOut <= markIn) {
      setError("インとアウトを置いてください");
      return;
    }
    apply((draft) => edit(draft, markIn, markOut));
  }
}

function endOf(sequence: Sequence): number {
  return rowsOf(sequence).reduce((max, row) => {
    return row.track.clips.reduce((inner, clip) => Math.max(inner, clip.timelineOut), max);
  }, 0);
}

function findClip(sequence: Sequence, id: string | undefined): Clip | undefined {
  if (!id) {
    return undefined;
  }
  return rowsOf(sequence).flatMap((row) => row.track.clips).find((clip) => clip.id === id);
}

function neighbor(track: Track, clip: Clip, side: "prev" | "next"): Clip | undefined {
  if (side === "prev") {
    return track.clips.find((item) => item.timelineOut === clip.timelineIn);
  }
  return track.clips.find((item) => item.timelineIn === clip.timelineOut);
}
