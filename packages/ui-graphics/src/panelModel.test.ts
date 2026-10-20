import assert from "node:assert/strict";
import test from "node:test";
import type { Clip, Sequence } from "@videocut/model";
import { LOWER_THIRD_EFFECT_ID, TITLE_EFFECT_ID, paramIds } from "./schema.ts";
import { graphicsPanelModel } from "./panelModel.ts";
import { msToTicks } from "./timing.ts";

function sequence(captions: Sequence["captions"]): Sequence {
  return {
    id: "seq-1",
    name: "seq",
    settings: {
      width: 1920,
      height: 1080,
      fps: { num: 30, den: 1 },
      sampleRate: 48_000,
      audioChannels: 2,
      startTimecodeTicks: 0,
    },
    videoTracks: [],
    audioTracks: [],
    captions,
  };
}

const title: Clip = {
  id: "clip-title",
  name: "Hello",
  kind: "title",
  timelineIn: 0,
  timelineOut: 1_000,
  sourceIn: 0,
  sourceOut: 1_000,
  speed: 1,
  reversed: false,
  enabled: true,
  labelColor: 0,
  effects: [
    {
      id: "fx-1",
      effectId: TITLE_EFFECT_ID,
      enabled: true,
      params: [{ id: paramIds.text, value: { type: "str", value: "Hello" } }],
    },
  ],
};

test("panel model lists caption timing in timeline order and reads the title", () => {
  const model = graphicsPanelModel(
    sequence([
      { id: "b", startTicks: msToTicks(5_000), endTicks: msToTicks(6_000), text: "Later" },
      { id: "a", startTicks: msToTicks(1_000), endTicks: msToTicks(2_000), text: "First" },
    ]),
    title,
  );
  assert.equal(model.emptySequence, false);
  assert.deepEqual(model.graphic, { role: "title", clipId: "clip-title", text: "Hello" });
  assert.deepEqual(
    model.captions.map((caption) => caption.text),
    ["First", "Later"],
  );
  assert.equal(model.captions[0]?.startLabel, "00:00:01.000");
  assert.equal(model.captions[0]?.endLabel, "00:00:02.000");
});

test("lower third fields expose both lines", () => {
  const lower: Clip = {
    ...title,
    id: "clip-lower",
    effects: [
      {
        id: "fx-2",
        effectId: LOWER_THIRD_EFFECT_ID,
        enabled: true,
        params: [
          { id: paramIds.primary, value: { type: "str", value: "Aoki" } },
          { id: paramIds.secondary, value: { type: "str", value: "Editor" } },
        ],
      },
    ],
  };
  const model = graphicsPanelModel(sequence([]), lower);
  assert.deepEqual(model.graphic, {
    role: "lowerThird",
    clipId: "clip-lower",
    primary: "Aoki",
    secondary: "Editor",
  });
});

test("a closed sequence and a non-title clip have empty graphic fields", () => {
  assert.equal(graphicsPanelModel(null, title).emptySequence, true);
  const media: Clip = { ...title, id: "media", kind: "media", effects: [] };
  const model = graphicsPanelModel(sequence([]), media);
  assert.deepEqual(model.graphic, { role: "none" });
  assert.deepEqual(model.captions, []);
});
