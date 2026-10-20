import { effectIds, type EffectInstance } from "@videocut/model";
import { describe, expect, it } from "vitest";

import { fromDisplay, toDisplay } from "./values";
import {
  addEffect,
  moveEffect,
  setKeyframeInterp,
  setParamValue,
  toggleKeyframe,
} from "./edit";

const opacity: EffectInstance = {
  id: "fx-opacity",
  effectId: effectIds.opacity,
  enabled: true,
  params: [{ id: "opacity", value: { type: "float", value: 1 }, keyframes: [] }],
};

describe("effect edits", () => {
  it("converts percent display without changing stored units", () => {
    expect(toDisplay("percent", 0.4)).toBe(40);
    expect(fromDisplay("percent", 40)).toBe(0.4);
  });

  it("writes a static value, then keys the playhead", () => {
    const written = setParamValue([opacity], "fx-opacity", "opacity", { type: "float", value: 0.4 }, 10);
    expect(written[0]?.params[0]?.value).toEqual({ type: "float", value: 0.4 });

    const keyed = toggleKeyframe(written, "fx-opacity", "opacity", 10);
    expect(keyed[0]?.params[0]?.keyframes).toEqual([
      { timeTicks: 10, value: { type: "float", value: 0.4 }, interp: "linear" },
    ]);

    const inserted = setParamValue(keyed, "fx-opacity", "opacity", { type: "float", value: 0.2 }, 20);
    expect(inserted[0]?.params[0]?.keyframes?.map((key) => key.timeTicks)).toEqual([10, 20]);

    const held = setKeyframeInterp(inserted, "fx-opacity", "opacity", 10, "hold");
    expect(held[0]?.params[0]?.keyframes?.[0]?.interp).toBe("hold");

    const cleared = toggleKeyframe(held, "fx-opacity", "opacity", 10);
    const again = toggleKeyframe(cleared, "fx-opacity", "opacity", 20);
    expect(again[0]?.params[0]?.keyframes).toEqual([]);
    expect(again[0]?.params[0]?.value).toEqual({ type: "float", value: 0.2 });
  });

  it("adds and reorders effects", () => {
    const withCrop = addEffect([opacity], effectIds.crop);
    expect(withCrop.map((effect) => effect.effectId)).toEqual([effectIds.opacity, effectIds.crop]);
    const moved = moveEffect(withCrop, withCrop[1]?.id ?? "", -1);
    expect(moved.map((effect) => effect.effectId)).toEqual([effectIds.crop, effectIds.opacity]);
  });
});
