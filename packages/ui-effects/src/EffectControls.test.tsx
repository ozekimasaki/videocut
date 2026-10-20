import { effectIds, type Clip, type EffectInstance, type Transition } from "@videocut/model";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => {
  cleanup();
});

import { EffectControls } from "./EffectControls";

const motion: EffectInstance = {
  id: "fx-motion",
  effectId: effectIds.motion,
  enabled: true,
  params: [
    { id: "anchor", value: { type: "point", value: { x: 0.5, y: 0.5 } } },
    { id: "position", value: { type: "point", value: { x: 0.5, y: 0.5 } } },
    { id: "scale", value: { type: "point", value: { x: 1, y: 1 } } },
    { id: "rotation", value: { type: "float", value: 0 } },
  ],
};

const opacity: EffectInstance = {
  id: "fx-opacity",
  effectId: effectIds.opacity,
  enabled: true,
  params: [
    {
      id: "opacity",
      value: { type: "float", value: 1 },
      keyframes: [
        { timeTicks: 0, value: { type: "float", value: 0 }, interp: "linear" },
        { timeTicks: 100, value: { type: "float", value: 1 }, interp: "linear" },
      ],
    },
  ],
};

function clip(effects: EffectInstance[], kind: Clip["kind"] = "media"): Clip {
  return {
    id: "clip-1",
    name: "Interview",
    kind,
    timelineIn: 1000,
    timelineOut: 5000,
    sourceIn: 0,
    sourceOut: 4000,
    speed: 1,
    reversed: false,
    enabled: true,
    labelColor: 0,
    effects,
  };
}

const transition: Transition = {
  id: "tr-1",
  effectId: effectIds.crossDissolve,
  centerTicks: 1000,
  durationTicks: 200,
  alignment: "center",
};

describe("EffectControls", () => {
  it("shows the interpolated value and edits the stack", () => {
    const onEffectsChange = vi.fn();
    render(
      <EffectControls
        clip={clip([motion, opacity], "adjustment")}
        playheadTicks={1040}
        onEffectsChange={onEffectsChange}
        transition={transition}
      />,
    );

    expect(screen.getByRole("region", { name: "Effect Controls" })).toBeTruthy();
    expect(screen.getByText(/Adjustment layer/)).toBeTruthy();
    expect(screen.getByTestId("evaluated-fx-opacity-opacity").textContent).toBe("40.0%");
    expect(screen.getByTestId("transition-blend").textContent).toContain("Outgoing 30.0%");
    expect(screen.getByTestId("transition-blend").textContent).toContain("Incoming 70.0%");

    fireEvent.click(screen.getByRole("checkbox", { name: "Opacity enabled" }));
    const disabled = onEffectsChange.mock.calls[0]?.[0] as EffectInstance[];
    expect(disabled.find((effect) => effect.id === "fx-opacity")?.enabled).toBe(false);

    fireEvent.change(screen.getByLabelText("Add effect"), { target: { value: effectIds.crop } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    const added = onEffectsChange.mock.calls[1]?.[0] as EffectInstance[];
    expect(added.map((effect) => effect.effectId)).toContain(effectIds.crop);

    fireEvent.click(screen.getByRole("button", { name: "Add keyframe for Rotation" }));
    const keyed = onEffectsChange.mock.calls[2]?.[0] as EffectInstance[];
    const rotation = keyed
      .find((effect) => effect.id === "fx-motion")
      ?.params.find((param) => param.id === "rotation");
    expect(rotation?.keyframes?.[0]).toMatchObject({ timeTicks: 40, interp: "linear" });

    fireEvent.change(screen.getByLabelText("Opacity value"), { target: { value: "25" } });
    const written = onEffectsChange.mock.calls[3]?.[0] as EffectInstance[];
    const opacityParam = written
      .find((effect) => effect.id === "fx-opacity")
      ?.params.find((param) => param.id === "opacity");
    expect(opacityParam?.keyframes?.find((key) => key.timeTicks === 40)?.value).toEqual({
      type: "float",
      value: 0.25,
    });
  });

  it("updates transition alignment", () => {
    const onTransitionChange = vi.fn();
    render(
      <EffectControls
        clip={clip([])}
        playheadTicks={0}
        onEffectsChange={vi.fn()}
        transition={transition}
        onTransitionChange={onTransitionChange}
      />,
    );
    expect(screen.getByText("No effects on this clip")).toBeTruthy();
    expect(screen.getByTestId("transition-blend").textContent).toBe("Outside transition");
    fireEvent.change(screen.getByLabelText("Transition alignment"), { target: { value: "end" } });
    expect(onTransitionChange.mock.calls[0]?.[0]).toMatchObject({ alignment: "end" });
  });
});
