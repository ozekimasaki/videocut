import assert from "node:assert/strict";
import test from "node:test";
import {
  dragCurvePoint,
  formatCurve,
  insertCurvePoint,
  parseCurve,
  removeCurvePoint,
  sampleCurve,
} from "./curves";

test("samples a piecewise-linear curve", () => {
  const parsed = parseCurve("0,0;0.5,0.25;1,1");
  assert.equal(parsed.ok, true);
  if (!parsed.ok) return;
  assert.equal(sampleCurve(parsed.points, 0.25), 0.125);
  assert.equal(sampleCurve(parsed.points, 0.75), 0.625);
  assert.equal(sampleCurve(parsed.points, -1), 0);
  assert.equal(sampleCurve(parsed.points, 2), 1);
});

test("sorts points and keeps the later duplicate", () => {
  const parsed = parseCurve("1,1;0,0;0.5,0.25;0.5,0.5");
  assert.equal(parsed.ok, true);
  if (!parsed.ok) return;
  assert.deepEqual(parsed.points[0], { x: 0, y: 0 });
  assert.equal(sampleCurve(parsed.points, 0.5), 0.5);
  assert.equal(formatCurve(parsed.points), "0,0;0.5,0.5;1,1");
});

test("rejects a malformed curve", () => {
  assert.equal(parseCurve("nope").ok, false);
  assert.equal(parseCurve("0,0,1").ok, false);
});

test("drag locks endpoints and insert refuses a crowded x", () => {
  const points = [
    { x: 0, y: 0 },
    { x: 0.5, y: 0.5 },
    { x: 1, y: 1 },
  ];
  assert.deepEqual(dragCurvePoint(points, 0, 0.4, 0.2)[0], { x: 0, y: 0.2 });
  assert.equal(insertCurvePoint(points, 0.5, 0.2).length, 3);
  assert.equal(insertCurvePoint(points, 0.25, 0.2).length, 4);
  assert.equal(removeCurvePoint(points, 0).length, 3);
  assert.equal(removeCurvePoint(points, 1).length, 2);
});
