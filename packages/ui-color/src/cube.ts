const MAX_LUT_SIZE = 128;

export type CubeInspection =
  | { ok: true; kind: "identity" }
  | { ok: true; kind: "1d" | "3d"; size: number; rows: number }
  | { ok: false; message: string };

/** Checks the header and row count of a `.cube` LUT. Sampling stays in the color crate. */
export function inspectCube(source: string): CubeInspection {
  const text = source.replace(/^\uFEFF/, "");
  if (!text.trim()) {
    return { ok: true, kind: "identity" };
  }

  let kind: "1d" | "3d" | null = null;
  let size = 0;
  let rows = 0;

  const lines = text.split(/\r?\n/);
  for (let index = 0; index < lines.length; index += 1) {
    const hash = lines[index].indexOf("#");
    const line = (hash === -1 ? lines[index] : lines[index].slice(0, hash)).trim();
    if (!line) {
      continue;
    }
    const tokens = line.split(/\s+/);
    if (isFloat(tokens[0])) {
      if (size === 0) {
        return { ok: false, message: `line ${index + 1}: data appears before the LUT size` };
      }
      if (tokens.length !== 3 || tokens.some((token) => !isFloat(token))) {
        return { ok: false, message: `line ${index + 1}: expected 3 sample values` };
      }
      rows += 1;
      continue;
    }
    const keyword = tokens[0].toUpperCase();
    if (keyword === "LUT_1D_SIZE" || keyword === "LUT_3D_SIZE") {
      if (kind) {
        return { ok: false, message: "a cube LUT declares one size" };
      }
      const parsed = parseSize(tokens, index);
      if (!parsed.ok) {
        return parsed;
      }
      size = parsed.size;
      kind = keyword === "LUT_1D_SIZE" ? "1d" : "3d";
    }
  }

  if (!kind) {
    return { ok: false, message: "missing LUT size" };
  }
  const expected = kind === "1d" ? size : size ** 3;
  if (rows !== expected) {
    return { ok: false, message: `expected ${expected} samples, found ${rows}` };
  }
  return { ok: true, kind, size, rows };
}

function parseSize(
  tokens: string[],
  lineIndex: number,
): { ok: true; size: number } | { ok: false; message: string } {
  if (tokens.length !== 2) {
    return { ok: false, message: `line ${lineIndex + 1}: LUT size needs one integer` };
  }
  const size = Number(tokens[1]);
  if (!Number.isInteger(size)) {
    return { ok: false, message: `line ${lineIndex + 1}: LUT size is not an integer` };
  }
  if (size < 2 || size > MAX_LUT_SIZE) {
    return { ok: false, message: `LUT size must be from 2 to ${MAX_LUT_SIZE}` };
  }
  return { ok: true, size };
}

function isFloat(token: string): boolean {
  const value = Number(token);
  return Number.isFinite(value);
}
