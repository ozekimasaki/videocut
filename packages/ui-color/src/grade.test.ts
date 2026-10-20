import assert from "node:assert/strict";
import test from "node:test";
import { effectIds } from "@videocut/model";
import {
  colorParams,
  createColorWheelsEffect,
  createCurvesEffect,
  createHslEffect,
  createLutEffect,
  isColorEffect,
  setFloatParam,
  toolForEffect,
} from "./grade";

test("color effect factories use the builtin ids", () => {
  assert.equal(createColorWheelsEffect().effectId, effectIds.colorWheels);
  assert.equal(createCurvesEffect().effectId, effectIds.curves);
  assert.equal(createHslEffect().effectId, effectIds.hsl);
  assert.equal(createLutEffect("LUT_1D_SIZE 2\n").effectId, effectIds.lut);
  assert.equal(toolForEffect(effectIds.colorWheels), "wheels");
  assert.equal(isColorEffect(effectIds.opacity), false);
});

test("setFloatParam keeps keyframes and the other parameters", () => {
  const effect = createColorWheelsEffect();
  const withKeys = {
    ...effect,
    params: effect.params.map((param) =>
      param.id === colorParams.shadowsMaster
        ? {
            ...param,
            keyframes: [{ timeTicks: 0, value: { type: "float" as const, value: 0 }, interp: "linear" as const }],
          }
        : param,
    ),
  };
  const next = setFloatParam(withKeys, colorParams.shadowsMaster, 0.25);
  const master = next.params.find((param) => param.id === colorParams.shadowsMaster);
  assert.equal(master?.value.type, "float");
  assert.equal(master?.value.type === "float" ? master.value.value : null, 0.25);
  assert.equal(master?.keyframes?.length, 1);
  assert.equal(next.params.length, effect.params.length);
});
