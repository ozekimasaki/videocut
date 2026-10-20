/** Matches `videocut_model::TICKS_PER_SECOND` and `@videocut/model`. */
export const TICKS_PER_SECOND = 254_016_000_000;

const TICKS = BigInt(TICKS_PER_SECOND);

/** Nearest millisecond, half away from zero for the positive timeline. */
export function ticksToMs(ticks: number): number {
  if (!Number.isFinite(ticks) || ticks <= 0) {
    return 0;
  }
  const value = BigInt(Math.trunc(ticks));
  return Number((value * 1000n + TICKS / 2n) / TICKS);
}

export function msToTicks(ms: number): number {
  if (!Number.isFinite(ms) || ms <= 0) {
    return 0;
  }
  return Number(BigInt(Math.trunc(ms)) * (TICKS / 1000n));
}

export function formatTicks(ticks: number): string {
  const total = Math.max(0, ticksToMs(ticks));
  const hours = Math.floor(total / 3_600_000);
  const minutes = Math.floor((total % 3_600_000) / 60_000);
  const seconds = Math.floor((total % 60_000) / 1_000);
  const millis = total % 1_000;
  return `${pad(hours, 2)}:${pad(minutes, 2)}:${pad(seconds, 2)}.${pad(millis, 3)}`;
}

/** Accepts `HH:MM:SS.mmm` and the comma fractional separator used by SRT. */
export function parseTimecode(raw: string): number | null {
  const match = /^(\d+):(\d{2}):(\d{2})[.,](\d{1,3})$/.exec(raw.trim());
  if (!match) {
    return null;
  }
  const hours = Number(match[1]);
  const minutes = Number(match[2]);
  const seconds = Number(match[3]);
  const fraction = match[4] ?? "";
  if (minutes > 59 || seconds > 59 || fraction.length === 0) {
    return null;
  }
  const scale = fraction.length === 1 ? 100 : fraction.length === 2 ? 10 : 1;
  const millis = Number(fraction) * scale;
  return msToTicks(hours * 3_600_000 + minutes * 60_000 + seconds * 1_000 + millis);
}

function pad(value: number, width: number): string {
  return String(value).padStart(width, "0");
}
