export const METER_FLOOR_DB = -60;
export const FADER_MIN_DB = -96;
export const FADER_MAX_DB = 12;
export const PAN_MIN = -1;
export const PAN_MAX = 1;

export function meterFraction(dbfs: number, floor = METER_FLOOR_DB): number {
  if (!Number.isFinite(dbfs) || floor >= 0) {
    return 0;
  }
  const clamped = Math.min(0, Math.max(floor, dbfs));
  return (clamped - floor) / -floor;
}

export function formatDb(db: number): string {
  if (!Number.isFinite(db)) {
    return "-inf dB";
  }
  const rounded = Math.round(db * 10) / 10;
  const text = rounded.toFixed(1);
  return `${rounded > 0 ? "+" : ""}${text} dB`;
}

export function formatPan(pan: number): string {
  if (!Number.isFinite(pan) || Math.abs(pan) < 0.005) {
    return "C";
  }
  const amount = Math.round(Math.abs(pan) * 100);
  return pan < 0 ? `L${amount}` : `R${amount}`;
}

export function clampFader(db: number): number {
  if (!Number.isFinite(db)) {
    return 0;
  }
  return Math.min(FADER_MAX_DB, Math.max(FADER_MIN_DB, db));
}

export function clampPan(pan: number): number {
  if (!Number.isFinite(pan)) {
    return 0;
  }
  return Math.min(PAN_MAX, Math.max(PAN_MIN, pan));
}
