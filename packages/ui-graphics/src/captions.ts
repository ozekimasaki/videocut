import type { Caption, Sequence } from "@videocut/model";
import type { EditResult } from "./title.ts";
import { parseTimecode } from "./timing.ts";

export function withCaptionText(caption: Caption, text: string): Caption {
  return { ...caption, text };
}

export function withCaptionTiming(
  caption: Caption,
  startTicks: number,
  endTicks: number,
): EditResult<Caption> {
  if (
    !Number.isFinite(startTicks) ||
    !Number.isFinite(endTicks) ||
    startTicks < 0 ||
    endTicks <= startTicks
  ) {
    return { ok: false, error: "終了は開始より後にしてください" };
  }
  return { ok: true, value: { ...caption, startTicks, endTicks } };
}

export function commitCaptionTiming(
  startDraft: string,
  endDraft: string,
): EditResult<{ startTicks: number; endTicks: number }> {
  const startTicks = parseTimecode(startDraft);
  const endTicks = parseTimecode(endDraft);
  if (startTicks === null || endTicks === null) {
    return { ok: false, error: "時刻は HH:MM:SS.mmm で入力してください" };
  }
  const timing = withCaptionTiming(
    { id: "", startTicks, endTicks, text: "" },
    startTicks,
    endTicks,
  );
  if (!timing.ok) {
    return timing;
  }
  return { ok: true, value: { startTicks, endTicks } };
}

export function updateCaptionText(
  sequence: Sequence,
  captionId: string,
  text: string,
): EditResult<Sequence> {
  return mapCaption(sequence, captionId, (caption) => ({
    ok: true,
    value: withCaptionText(caption, text),
  }));
}

export function updateCaptionTiming(
  sequence: Sequence,
  captionId: string,
  startTicks: number,
  endTicks: number,
): EditResult<Sequence> {
  return mapCaption(sequence, captionId, (caption) => withCaptionTiming(caption, startTicks, endTicks));
}

function mapCaption(
  sequence: Sequence,
  captionId: string,
  edit: (caption: Caption) => EditResult<Caption>,
): EditResult<Sequence> {
  const captions = sequence.captions ?? [];
  const index = captions.findIndex((caption) => caption.id === captionId);
  if (index < 0) {
    return { ok: false, error: "字幕が見つかりません" };
  }
  const nextCaption = edit(captions[index]);
  if (!nextCaption.ok) {
    return nextCaption;
  }
  const next = captions.slice();
  next[index] = nextCaption.value;
  return { ok: true, value: { ...sequence, captions: next } };
}
