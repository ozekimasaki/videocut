import type { MonitorPicture } from "../host/playhead";
import { labelColor } from "./colors";

export function drawMonitor(canvas: HTMLCanvasElement, picture: MonitorPicture, image: CanvasImageSource | null): void {
  const rect = canvas.getBoundingClientRect();
  const ratio = window.devicePixelRatio || 1;
  const width = Math.max(1, Math.round(rect.width * ratio));
  const height = Math.max(1, Math.round(rect.height * ratio));
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  const context = canvas.getContext("2d");
  if (!context) return;
  const cssWidth = Math.max(1, rect.width);
  const cssHeight = Math.max(1, rect.height);
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  context.clearRect(0, 0, cssWidth, cssHeight);
  if (image) {
    context.drawImage(image, 0, 0, cssWidth, cssHeight);
  } else {
    context.fillStyle = picture.clipName ? labelColor(picture.labelColor) : "#101214";
    context.fillRect(0, 0, cssWidth, cssHeight);
    if (picture.clipName) {
      const shade = context.createLinearGradient(0, 0, cssWidth, cssHeight);
      shade.addColorStop(0, "rgba(255,255,255,0.16)");
      shade.addColorStop(1, "rgba(0,0,0,0.28)");
      context.fillStyle = shade;
      context.fillRect(0, 0, cssWidth, cssHeight);
      const band = cssWidth * Math.min(1, Math.max(0, picture.progress));
      context.fillStyle = "rgba(20,18,16,0.35)";
      context.fillRect(band, 0, 2, cssHeight);
    }
  }
  const caption = picture.overlayText || (picture.kind === "title" ? picture.clipName : null);
  if (caption) {
    context.fillStyle = "rgba(12,12,12,0.45)";
    context.fillRect(0, cssHeight * 0.38, cssWidth, 42);
    context.fillStyle = "#f6f3ee";
    context.font = "600 22px sans-serif";
    context.textAlign = "center";
    context.textBaseline = "middle";
    context.fillText(caption, cssWidth / 2, cssHeight * 0.38 + 21, cssWidth - 24);
  }
}
