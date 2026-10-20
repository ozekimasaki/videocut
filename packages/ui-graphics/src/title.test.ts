import assert from "node:assert/strict";
import test from "node:test";
import type { Clip } from "@videocut/model";
import { LOWER_THIRD_EFFECT_ID, TITLE_EFFECT_ID, paramIds } from "./schema.ts";
import { graphicRole, readTitleText, withLowerThirdText, withTitleText } from "./title.ts";

function clip(kind: Clip["kind"], effects: Clip["effects"]): Clip {
  return {
    id: "clip-1",
    name: "original",
    kind,
    timelineIn: 0,
    timelineOut: 1_000,
    sourceIn: 0,
    sourceOut: 1_000,
    speed: 1,
    reversed: false,
    enabled: true,
    labelColor: 0,
    effects,
  };
}

function titleClip(): Clip {
  return clip("title", [
    {
      id: "fx-1",
      effectId: TITLE_EFFECT_ID,
      enabled: true,
      params: [
        { id: paramIds.text, value: { type: "str", value: "Before" } },
        { id: paramIds.fontFamily, value: { type: "str", value: "sans-serif" } },
      ],
    },
  ]);
}

test("title text edit keeps the other parameters and the clip name", () => {
  const updated = withTitleText(titleClip(), "After");
  assert.equal(updated.ok, true);
  if (!updated.ok) {
    return;
  }
  assert.equal(updated.value.name, "After");
  assert.equal(readTitleText(updated.value), "After");
  const font = updated.value.effects?.[0]?.params.find((param) => param.id === paramIds.fontFamily);
  assert.deepEqual(font?.value, { type: "str", value: "sans-serif" });
});

test("lower third edits both lines and titles reject the wrong setter", () => {
  const lower = clip("title", [
    {
      id: "fx-2",
      effectId: LOWER_THIRD_EFFECT_ID,
      enabled: true,
      params: [
        { id: paramIds.primary, value: { type: "str", value: "Name" } },
        { id: paramIds.secondary, value: { type: "str", value: "Role" } },
        { id: paramIds.accent, value: { type: "color", value: { r: 1, g: 0, b: 0, a: 1 } } },
      ],
    },
  ]);
  assert.equal(graphicRole(lower), "lowerThird");
  const updated = withLowerThirdText(lower, "New", "Updated");
  assert.equal(updated.ok, true);
  if (!updated.ok) {
    return;
  }
  assert.equal(updated.value.name, "New");
  const primary = updated.value.effects?.[0]?.params.find((param) => param.id === paramIds.primary);
  const secondary = updated.value.effects?.[0]?.params.find(
    (param) => param.id === paramIds.secondary,
  );
  assert.deepEqual(primary?.value, { type: "str", value: "New" });
  assert.deepEqual(secondary?.value, { type: "str", value: "Updated" });
  assert.equal(withTitleText(lower, "nope").ok, false);
  assert.equal(withLowerThirdText(titleClip(), "nope", "nope").ok, false);
});

test("a media clip is not a graphic", () => {
  const media = clip("media", []);
  assert.equal(graphicRole(media), null);
  assert.equal(withTitleText(media, "x").ok, false);
});
