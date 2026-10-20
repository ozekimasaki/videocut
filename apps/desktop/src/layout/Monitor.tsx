import { useEffect, useRef, useState, type ReactNode } from "react";
import { TICKS_PER_SECOND, type Rational } from "@videocut/model";
import { requestFrame } from "../host/desktop-bridge";
import type { MonitorPicture } from "../host/playhead";
import { formatTimecode } from "../model/timecode";
import { PauseIcon, PlayIcon, StepBackIcon, StepForwardIcon } from "../ui/icons";
import { drawMonitor } from "../ui/draw-monitor";

export function Monitor({
  picture,
  fps,
  focused,
  playing,
  testId,
  onFocus,
  onTogglePlay,
  onStep,
  extra,
}: {
  picture: MonitorPicture;
  fps: Rational;
  focused: boolean;
  playing: boolean;
  testId: string;
  onFocus: () => void;
  onTogglePlay: () => void;
  onStep: (direction: -1 | 1) => void;
  extra?: ReactNode;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [image, setImage] = useState<HTMLImageElement | null>(null);
  const unsupported = useRef(false);
  const lastRequest = useRef("");

  useEffect(() => {
    unsupported.current = false;
    lastRequest.current = "";
    setImage(null);
  }, [picture.mediaPath]);

  useEffect(() => {
    if (!picture.mediaPath || unsupported.current) return;
    const quantum = Math.max(1, Math.round((TICKS_PER_SECOND * fps.den) / fps.num));
    const frame = Math.floor(picture.frameTicks / quantum);
    const key = `${picture.mediaPath}:${frame}`;
    if (lastRequest.current === key) return;
    lastRequest.current = key;
    let cancelled = false;
    void requestFrame(picture.mediaPath, frame * quantum, picture.width, picture.height)
      .then((png) => {
        if (cancelled) return;
        if (!png) {
          unsupported.current = true;
          setImage(null);
          return;
        }
        const next = new Image();
        next.onload = () => {
          if (!cancelled) setImage(next);
        };
        next.src = `data:image/png;base64,${png}`;
      })
      .catch(() => {
        unsupported.current = true;
      });
    return () => {
      cancelled = true;
    };
  }, [fps.den, fps.num, picture.frameTicks, picture.height, picture.mediaPath, picture.width]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    drawMonitor(canvas, picture, image);
  }, [image, picture]);

  const timecode = formatTimecode(picture.timecodeTicks, fps);
  const duration = formatTimecode(picture.durationTicks, fps);
  return (
    <section
      className={focused ? "monitor is-focused" : "monitor"}
      data-testid={testId}
      data-playing={playing ? "yes" : "no"}
      onPointerDown={onFocus}
    >
      <header className="monitor__title">
        <span>{picture.title}</span>
        <span className="monitor__clip">{picture.clipName ?? (picture.title === "ソース" ? "ソースなし" : "クリップなし")}</span>
      </header>
      <div className="monitor__stage">
        <div className="monitor__frame" style={{ aspectRatio: `${picture.width} / ${picture.height}` }}>
          <canvas ref={canvasRef} />
        </div>
      </div>
      <footer className="monitor__transport">
        <button type="button" aria-label="1フレーム戻る" onClick={() => onStep(-1)}>
          <StepBackIcon />
        </button>
        <button type="button" aria-label={playing ? "停止" : "再生"} data-testid={`${testId}-play`} onClick={onTogglePlay}>
          {playing ? <PauseIcon /> : <PlayIcon />}
        </button>
        <button type="button" aria-label="1フレーム進む" onClick={() => onStep(1)}>
          <StepForwardIcon />
        </button>
        <span className="timecode" data-testid={`${testId}-timecode`}>
          {timecode}
        </span>
        <span className="monitor__duration">{duration}</span>
        {extra}
      </footer>
    </section>
  );
}
