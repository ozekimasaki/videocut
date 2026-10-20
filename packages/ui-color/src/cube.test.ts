import assert from "node:assert/strict";
import test from "node:test";
import { inspectCube } from "./cube";

const IDENTITY = `
# comment
TITLE "grade"
LUT_3D_SIZE 2
0 0 0
1 0 0
0 1 0
1 1 0
0 0 1
1 0 1
0 1 1
1 1 1
`;

test("inspects a 3D cube and an empty identity", () => {
  const inspected = inspectCube(IDENTITY);
  assert.deepEqual(inspected, { ok: true, kind: "3d", size: 2, rows: 8 });
  assert.deepEqual(inspectCube("  "), { ok: true, kind: "identity" });
});

test("rejects incomplete and dual-size cubes", () => {
  assert.equal(inspectCube("LUT_3D_SIZE 2\n0 0 0\n").ok, false);
  assert.equal(inspectCube("LUT_1D_SIZE 2\nLUT_3D_SIZE 2\n").ok, false);
  assert.equal(inspectCube("0 0 0\n").ok, false);
  const oneD = inspectCube("LUT_1D_SIZE 2\n0 0 0\n1 1 1\n");
  assert.deepEqual(oneD, { ok: true, kind: "1d", size: 2, rows: 2 });
});
