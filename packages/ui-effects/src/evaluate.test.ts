import { effectIds, type Param, type Transition } from "@videocut/model";
import { describe, expect, it } from "vitest";

import { createEffect, defaultVideoStack } from "./catalog";
import {
  applyAffine,
  bezierEase,
  blendWeights,
  evaluateParam,
  evaluateStack,
  transitionBounds,
  transitionProgress,
} from "./evaluate";
import { formatParamValue } from "./format";

function floatParam(value: number, keys: Param["keyframes"]): Param {
  return { id: "opacity", value: { type: "float", value }, keyframes: keys };
}

function close(actual: number, expected: number) {
  expect(Math.abs(actual - expected)).toBeLessThan(1e-9);
}

describe("keyframe evaluation", () => {
  it("matches linear, hold, and bezier", () => {
    const param = (interp: "linear" | "hold" | "bezier"): Param =>
      floatParam(0, [
        { timeTicks: 0, value: { type: "float", value: 0 }, interp },
        { timeTicks: 100, value: { type: "float", value: 100 }, interp: "linear" },
      ]);
    const at = (interp: "linear" | "hold" | "bezier") => {
      const value = evaluateParam(param(interp), 40);
      if (value.type !== "float") throw new Error("float");
      return value.value;
    };
    close(at("linear"), 40);
    close(at("hold"), 0);
    close(at("bezier"), 35.2);
    close(bezierEase(0.25), 0.15625);
  });

  it("clamps outside the key range and keeps the later duplicate", () => {
    const value = evaluateParam(
      floatParam(5, [
        { timeTicks: 100, value: { type: "float", value: 10 }, interp: "linear" },
        { timeTicks: 0, value: { type: "float", value: 0 }, interp: "linear" },
        { timeTicks: 0, value: { type: "float", value: 4 }, interp: "linear" },
      ]),
      50,
    );
    if (value.type !== "float") throw new Error("float");
    close(value.value, 7);
  });

  it("interpolates points and holds text", () => {
    const point = evaluateParam(
      {
        id: "position",
        value: { type: "point", value: { x: 0, y: 0 } },
        keyframes: [
          { timeTicks: 0, value: { type: "point", value: { x: 0, y: 0 } }, interp: "linear" },
          { timeTicks: 10, value: { type: "point", value: { x: 10, y: 20 } }, interp: "linear" },
        ],
      },
      5,
    );
    if (point.type !== "point") throw new Error("point");
    close(point.value.x, 5);
    close(point.value.y, 10);

    const text = evaluateParam(
      {
        id: "label",
        value: { type: "str", value: "a" },
        keyframes: [
          { timeTicks: 0, value: { type: "str", value: "a" }, interp: "linear" },
          { timeTicks: 10, value: { type: "str", value: "b" }, interp: "linear" },
        ],
      },
      9,
    );
    expect(text).toEqual({ type: "str", value: "a" });
  });
});

describe("effect stack", () => {
  it("leaves the default stack at identity", () => {
    const evaluated = evaluateStack(defaultVideoStack(), 0);
    const point = applyAffine(evaluated.affine, 0.25, 0.75);
    close(point.x, 0.25);
    close(point.y, 0.75);
    close(evaluated.opacity, 1);
    expect(evaluated.crop).toEqual({ left: 0, top: 0, right: 0, bottom: 0 });
  });

  it("multiplies opacity, ignores a disabled effect, and intersects crop", () => {
    const half = createEffect(effectIds.opacity);
    const quarter = createEffect(effectIds.opacity);
    if (!half || !quarter) throw new Error("opacity");
    half.params[0] = { ...half.params[0], id: "opacity", value: { type: "float", value: 0.5 } };
    quarter.params[0] = { ...quarter.params[0], id: "opacity", value: { type: "float", value: 0.5 } };
    close(evaluateStack([half, quarter], 0).opacity, 0.25);
    quarter.enabled = false;
    close(evaluateStack([half, quarter], 0).opacity, 0.5);

    const wide = createEffect(effectIds.crop);
    const tight = createEffect(effectIds.crop);
    if (!wide || !tight) throw new Error("crop");
    wide.params = wide.params.map((param) =>
      param.id === "left" || param.id === "right"
        ? { ...param, value: { type: "float" as const, value: param.id === "left" ? 0.1 : 0.1 } }
        : param,
    );
    tight.params = tight.params.map((param) => {
      if (param.id === "left") return { ...param, value: { type: "float" as const, value: 0.25 } };
      if (param.id === "top") return { ...param, value: { type: "float" as const, value: 0.2 } };
      return param;
    });
    const crop = evaluateStack([wide, tight], 0).crop;
    close(crop.left, 0.25);
    close(crop.top, 0.2);
    close(crop.right, 0.1);
  });

  it("rotates the top center to the right center", () => {
    const motion = createEffect(effectIds.motion);
    if (!motion) throw new Error("motion");
    motion.params = motion.params.map((param) =>
      param.id === "rotation" ? { ...param, value: { type: "float", value: 90 } } : param,
    );
    const point = applyAffine(evaluateStack([motion], 0).affine, 0.5, 0);
    close(point.x, 1);
    close(point.y, 0.5);
  });
});

describe("transitions", () => {
  const transition = (alignment: Transition["alignment"]): Transition => ({
    id: "tr-1",
    effectId: effectIds.crossDissolve,
    centerTicks: 1000,
    durationTicks: 200,
    alignment,
  });

  it("places center, start, and end windows", () => {
    expect(transitionBounds(transition("center"))).toEqual({ start: 900, end: 1100 });
    expect(transitionBounds(transition("start"))).toEqual({ start: 1000, end: 1200 });
    expect(transitionBounds(transition("end"))).toEqual({ start: 800, end: 1000 });
    expect(transitionProgress(transition("center"), 950)).toBeCloseTo(0.25);
    expect(transitionProgress(transition("center"), 1100)).toBeNull();
  });

  it("weights dissolve, dip, and wipe", () => {
    const cross = blendWeights(effectIds.crossDissolve, 0.25);
    close(cross.outgoing, 0.75);
    close(cross.incoming, 0.25);
    const black = blendWeights(effectIds.dipToBlack, 0.5);
    close(black.dip, 1);
    expect(black.dipColor).toBe("black");
    const white = blendWeights(effectIds.dipToWhite, 0.75);
    close(white.incoming, 0.5);
    expect(white.dipColor).toBe("white");
    close(blendWeights(effectIds.wipe, 0.4).wipeEdge, 0.4);
    expect(blendWeights("custom.iris", 0.4).recognized).toBe(false);
    expect(formatParamValue("opacity", { type: "float", value: 0.352 })).toBe("35.2%");
  });
});
