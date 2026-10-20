import type { TimelineTool } from "@videocut/ui-timeline";
import type { ToolId } from "./types";

export function toTimelineTool(tool: ToolId): TimelineTool {
  switch (tool) {
    case "select":
    case "trackSelectForward":
    case "trackSelectBackward":
    case "pen":
      return "select";
    case "razor":
      return "razor";
    case "ripple":
      return "ripple";
    case "rolling":
      return "roll";
    case "slip":
      return "slip";
    case "slide":
      return "slide";
    default: {
      const unknown: never = tool;
      return unknown;
    }
  }
}

export function fromTimelineTool(tool: TimelineTool): ToolId {
  switch (tool) {
    case "select":
      return "select";
    case "razor":
      return "razor";
    case "ripple":
      return "ripple";
    case "roll":
      return "rolling";
    case "slip":
      return "slip";
    case "slide":
      return "slide";
    default: {
      const unknown: never = tool;
      throw new Error(String(unknown));
    }
  }
}
