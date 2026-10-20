export interface CurvePoint {
  x: number;
  y: number;
}

export type CurveParse =
  | { ok: true; points: CurvePoint[] }
  | { ok: false; message: string };

export const CURVE_IDENTITY = "0,0;1,1";

export function identityPoints(): CurvePoint[] {
  return [
    { x: 0, y: 0 },
    { x: 1, y: 1 },
  ];
}

/** `x,y` pairs separated by `;`. Same grammar as the Rust curve parser. */
export function parseCurve(source: string): CurveParse {
  const trimmed = source.trim();
  if (!trimmed) {
    return { ok: true, points: identityPoints() };
  }
  const points: CurvePoint[] = [];
  const parts = trimmed.split(";");
  for (let index = 0; index < parts.length; index += 1) {
    const part = parts[index].trim();
    if (!part) {
      continue;
    }
    const pieces = part.split(",").map((item) => item.trim());
    if (pieces.length > 2) {
      return { ok: false, message: `point ${index} has extra values` };
    }
    if (pieces.length !== 2 || pieces[0] === "" || pieces[1] === "") {
      return { ok: false, message: `point ${index} needs x and y` };
    }
    const x = Number(pieces[0]);
    const y = Number(pieces[1]);
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
      return { ok: false, message: `point ${index} is not a number` };
    }
    points.push({ x, y });
  }
  if (points.length === 0) {
    return { ok: false, message: "no control points" };
  }
  points.sort((a, b) => a.x - b.x);
  const collapsed: CurvePoint[] = [];
  for (const point of points) {
    const previous = collapsed[collapsed.length - 1];
    if (previous && Math.abs(previous.x - point.x) <= 1e-6) {
      collapsed[collapsed.length - 1] = point;
    } else {
      collapsed.push(point);
    }
  }
  return { ok: true, points: collapsed };
}

export function formatCurve(points: readonly CurvePoint[]): string {
  return points.map((point) => `${point.x},${point.y}`).join(";");
}

export function sampleCurve(points: readonly CurvePoint[], x: number): number {
  if (points.length === 0) {
    return x;
  }
  if (points.length === 1 || x <= points[0].x) {
    return points[0].y;
  }
  const last = points[points.length - 1];
  if (x >= last.x) {
    return last.y;
  }
  for (let index = 0; index < points.length - 1; index += 1) {
    const left = points[index];
    const right = points[index + 1];
    if (x >= left.x && x <= right.x) {
      const span = right.x - left.x;
      const t = Math.abs(span) < 1e-8 ? 0 : (x - left.x) / span;
      return left.y + (right.y - left.y) * t;
    }
  }
  return last.y;
}

export function insertCurvePoint(points: readonly CurvePoint[], x: number, y: number): CurvePoint[] {
  const point = { x: clamp(x, 0, 1), y: clamp(y, -0.5, 1.5) };
  if (points.some((existing) => Math.abs(existing.x - point.x) < 0.01)) {
    return points.map((existing) => ({ ...existing }));
  }
  return [...points, point].sort((a, b) => a.x - b.x);
}

export function removeCurvePoint(points: readonly CurvePoint[], index: number): CurvePoint[] {
  if (points.length <= 2 || index <= 0 || index >= points.length - 1) {
    return points.map((point) => ({ ...point }));
  }
  return points.filter((_, pointIndex) => pointIndex !== index);
}

export function dragCurvePoint(
  points: readonly CurvePoint[],
  index: number,
  x: number,
  y: number,
): CurvePoint[] {
  if (index < 0 || index >= points.length) {
    return points.map((point) => ({ ...point }));
  }
  const next = points.map((point) => ({ ...point }));
  const yClamped = clamp(y, -0.5, 1.5);
  if (index === 0) {
    next[0] = { x: 0, y: yClamped };
    return next;
  }
  if (index === next.length - 1) {
    next[index] = { x: 1, y: yClamped };
    return next;
  }
  const minX = next[index - 1].x + 0.001;
  const maxX = next[index + 1].x - 0.001;
  next[index] = { x: clamp(x, minX, maxX), y: yClamped };
  return next;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
