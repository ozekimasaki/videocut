import type { Clip, Sequence } from "@videocut/model";
import { graphicRole, readLowerThird, readTitleText } from "./title.ts";
import { formatTicks } from "./timing.ts";

export type GraphicFields =
  | { role: "title"; clipId: string; text: string }
  | { role: "lowerThird"; clipId: string; primary: string; secondary: string }
  | { role: "none" };

export type CaptionRowModel = {
  id: string;
  text: string;
  startTicks: number;
  endTicks: number;
  startLabel: string;
  endLabel: string;
};

export type GraphicsPanelModel = {
  emptySequence: boolean;
  graphic: GraphicFields;
  captions: CaptionRowModel[];
};

export function graphicsPanelModel(
  sequence: Sequence | null,
  selectedClip: Clip | null,
): GraphicsPanelModel {
  if (!sequence) {
    return { emptySequence: true, graphic: { role: "none" }, captions: [] };
  }
  const captions = [...(sequence.captions ?? [])]
    .sort(
      (left, right) =>
        left.startTicks - right.startTicks ||
        left.endTicks - right.endTicks ||
        left.id.localeCompare(right.id),
    )
    .map((caption) => ({
      id: caption.id,
      text: caption.text,
      startTicks: caption.startTicks,
      endTicks: caption.endTicks,
      startLabel: formatTicks(caption.startTicks),
      endLabel: formatTicks(caption.endTicks),
    }));
  return {
    emptySequence: false,
    graphic: graphicFields(selectedClip),
    captions,
  };
}

function graphicFields(clip: Clip | null): GraphicFields {
  if (!clip) {
    return { role: "none" };
  }
  const role = graphicRole(clip);
  switch (role) {
    case "title":
      return { role: "title", clipId: clip.id, text: readTitleText(clip) ?? "" };
    case "lowerThird": {
      const text = readLowerThird(clip);
      return {
        role: "lowerThird",
        clipId: clip.id,
        primary: text?.primary ?? "",
        secondary: text?.secondary ?? "",
      };
    }
    case null:
      return { role: "none" };
    default: {
      const unreachable: never = role;
      return unreachable;
    }
  }
}
