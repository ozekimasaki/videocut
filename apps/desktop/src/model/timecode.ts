import { TICKS_PER_SECOND, type Rational } from "@videocut/model";

export function seconds(value: number): number {
  return Math.round(value * TICKS_PER_SECOND);
}

export function ticksPerFrame(fps: Rational): number {
  if (fps.num <= 0 || fps.den <= 0) return 1;
  return Math.max(1, Math.round((TICKS_PER_SECOND * fps.den) / fps.num));
}

export function ticksToPixels(ticks: number, pxPerSecond: number): number {
  return (ticks / TICKS_PER_SECOND) * pxPerSecond;
}

export function pixelsToTicks(pixels: number, pxPerSecond: number): number {
  if (pxPerSecond <= 0) return 0;
  return Math.round((pixels / pxPerSecond) * TICKS_PER_SECOND);
}

export function formatTimecode(ticks: number, fps: Rational): string {
  const sign = ticks < 0 ? "-" : "";
  const abs = Math.abs(ticks);
  const framesPerSecond = Math.max(1, Math.round(fps.num / Math.max(1, fps.den)));
  const totalFrames = Math.floor((abs * fps.num) / (TICKS_PER_SECOND * Math.max(1, fps.den)));
  const frames = totalFrames % framesPerSecond;
  const totalSeconds = Math.floor(totalFrames / framesPerSecond);
  const secs = totalSeconds % 60;
  const minutes = Math.floor(totalSeconds / 60) % 60;
  const hours = Math.floor(totalSeconds / 3600);
  return `${sign}${pad(hours)}:${pad(minutes)}:${pad(secs)}:${pad(frames)}`;
}

function pad(value: number): string {
  return value.toString().padStart(2, "0");
}
