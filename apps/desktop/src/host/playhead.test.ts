import { TICKS_PER_SECOND, type Sequence } from "@videocut/model";
import { describe, expect, it } from "vitest";
import { createDemoProject } from "../model/project-factory";
import { programPicture, snapTicks, sourceTicksAt, topClipAt } from "./playhead";

function demoSequence(): Sequence {
  const sequence = createDemoProject().sequences[0];
  if (!sequence) throw new Error("missing sequence");
  return sequence;
}

describe("topClipAt", () => {
  it("prefers the higher video track", () => {
    const sequence = demoSequence();
    const atStart = topClipAt(sequence, 0);
    expect(atStart?.clip.kind).toBe("title");
    const afterTitle = topClipAt(sequence, TICKS_PER_SECOND * 4);
    expect(afterTitle?.clip.name).toBe("カラー A");
    const second = topClipAt(sequence, TICKS_PER_SECOND * 7);
    expect(second?.clip.name).toBe("カラー B");
  });
});

describe("sourceTicksAt", () => {
  it("maps forward and reversed clips", () => {
    const clip = {
      id: "c",
      name: "c",
      kind: "media" as const,
      timelineIn: 0,
      timelineOut: 100,
      sourceIn: 10,
      sourceOut: 110,
      speed: 1,
      reversed: false,
      enabled: true,
      labelColor: 0,
    };
    expect(sourceTicksAt(clip, 25)).toBe(35);
    expect(sourceTicksAt({ ...clip, reversed: true }, 25)).toBe(85);
  });
});

describe("programPicture", () => {
  it("names the clip under the playhead", () => {
    const project = createDemoProject();
    const picture = programPicture(project, project.sequences[0] ?? null, 0);
    expect(picture.clipName).toBe("タイトル");
    expect(picture.title).toBe("プログラム");
  });
});

describe("snapTicks", () => {
  it("snaps inside the threshold and keeps distant times", () => {
    expect(snapTicks(12, [0, 10, 40], 3)).toBe(10);
    expect(snapTicks(12, [0, 40], 1)).toBe(12);
  });
});
