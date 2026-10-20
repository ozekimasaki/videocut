import { TICKS_PER_SECOND } from "@videocut/model";
import { describe, expect, it } from "vitest";
import { formatTimecode, pixelsToTicks, ticksPerFrame, ticksToPixels } from "./timecode";

const fps = { num: 30, den: 1 };

describe("formatTimecode", () => {
  it("formats zero, one frame, and one second", () => {
    expect(formatTimecode(0, fps)).toBe("00:00:00:00");
    expect(formatTimecode(ticksPerFrame(fps), fps)).toBe("00:00:00:01");
    expect(formatTimecode(TICKS_PER_SECOND, fps)).toBe("00:00:01:00");
    expect(formatTimecode(TICKS_PER_SECOND / 2, fps)).toBe("00:00:00:15");
  });
});

describe("pixel conversion", () => {
  it("round-trips a whole second", () => {
    const px = ticksToPixels(TICKS_PER_SECOND, 80);
    expect(px).toBe(80);
    expect(pixelsToTicks(px, 80)).toBe(TICKS_PER_SECOND);
  });
});
