import { describe, expect, it } from "vitest";
import { fromTimelineTool, toTimelineTool } from "./timeline-tool";

describe("timeline tool adapter", () => {
  it("maps the shell rolling tool onto the timeline roll tool", () => {
    expect(toTimelineTool("rolling")).toBe("roll");
    expect(fromTimelineTool("roll")).toBe("rolling");
  });

  it("keeps the shared tools aligned", () => {
    for (const tool of ["select", "razor", "ripple", "slip", "slide"] as const) {
      expect(fromTimelineTool(toTimelineTool(tool))).toBe(tool);
    }
  });
});
