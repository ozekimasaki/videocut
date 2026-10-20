import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ColorPanel } from "./ColorPanel";
import { createColorWheelsEffect, createCurvesEffect, createHslEffect, createLutEffect } from "./grade";

test("renders wheels, curves, hsl, lut, and scopes", () => {
  const wheels = renderToStaticMarkup(
    <ColorPanel effects={[createColorWheelsEffect()]} onChange={() => undefined} />,
  );
  assert.match(wheels, /Shadows/);
  assert.match(wheels, /Midtones/);
  assert.match(wheels, /Highlights/);
  assert.match(wheels, /Waveform/);
  assert.match(wheels, /Parade/);

  const curves = renderToStaticMarkup(
    <ColorPanel effects={[createCurvesEffect()]} onChange={() => undefined} />,
  );
  assert.match(curves, /Master/);
  assert.match(curves, /Red/);

  const hsl = renderToStaticMarkup(
    <ColorPanel effects={[createHslEffect()]} onChange={() => undefined} />,
  );
  assert.match(hsl, /Hue width/);
  assert.match(hsl, /Softness/);

  const lut = renderToStaticMarkup(
    <ColorPanel effects={[createLutEffect()]} onChange={() => undefined} />,
  );
  assert.match(lut, /Identity \(no LUT\)/);
  assert.match(lut, /LUT intensity/);

  const empty = renderToStaticMarkup(<ColorPanel effects={[]} onChange={() => undefined} />);
  assert.match(empty, /No color effects on this clip/);
  assert.match(empty, /No frame for the scopes/);
});
