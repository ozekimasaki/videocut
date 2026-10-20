import type { ToolId } from "./types";

export interface KeyInput {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

export type KeyAction =
  | { type: "tool"; tool: ToolId }
  | { type: "undo" }
  | { type: "redo" }
  | { type: "shuttle-forward" }
  | { type: "shuttle-reverse" }
  | { type: "shuttle-stop" }
  | { type: "toggle-play" }
  | { type: "mark-in" }
  | { type: "mark-out" }
  | { type: "trim-previous" }
  | { type: "trim-next" }
  | { type: "step"; direction: -1 | 1 }
  | { type: "ripple-delete" }
  | { type: "save" };

const TOOL_KEYS = {
  v: "select",
  a: "trackSelectForward",
  b: "ripple",
  n: "rolling",
  c: "razor",
  y: "slip",
  u: "slide",
  p: "pen",
} as const satisfies Record<string, ToolId>;

type ToolKey = keyof typeof TOOL_KEYS;

function isToolKey(key: string): key is ToolKey {
  return Object.prototype.hasOwnProperty.call(TOOL_KEYS, key);
}

export function actionFromKey(input: KeyInput): KeyAction | null {
  if (input.altKey) return null;
  const key = input.key.toLowerCase();
  const modified = input.ctrlKey || input.metaKey;
  if (modified && key === "z" && input.shiftKey) return { type: "redo" };
  if (modified && key === "z") return { type: "undo" };
  if (modified && key === "y") return { type: "redo" };
  if (modified && key === "s") return { type: "save" };
  if (modified) return null;
  if (input.shiftKey && key === "a") return { type: "tool", tool: "trackSelectBackward" };
  if (input.shiftKey && key === "arrowleft") return { type: "step", direction: -1 };
  if (input.shiftKey && key === "arrowright") return { type: "step", direction: 1 };
  if (input.shiftKey) return null;
  if (key === "arrowleft") return { type: "step", direction: -1 };
  if (key === "arrowright") return { type: "step", direction: 1 };
  if (key === " " || key === "delete" || key === "backspace") {
    if (key === " ") return { type: "toggle-play" };
    return { type: "ripple-delete" };
  }
  if (isToolKey(key)) return toolAction(key);
  switch (key) {
    case "j":
      return { type: "shuttle-reverse" };
    case "k":
      return { type: "shuttle-stop" };
    case "l":
      return { type: "shuttle-forward" };
    case "i":
      return { type: "mark-in" };
    case "o":
      return { type: "mark-out" };
    case "q":
      return { type: "trim-previous" };
    case "w":
      return { type: "trim-next" };
    default:
      return null;
  }
}

function toolAction(key: ToolKey): KeyAction {
  switch (key) {
    case "v":
    case "a":
    case "b":
    case "n":
    case "c":
    case "y":
    case "u":
    case "p":
      return { type: "tool", tool: TOOL_KEYS[key] };
    default: {
      const unknown: never = key;
      return unknown;
    }
  }
}

export function nextShuttleSpeed(current: number, sameDirection: boolean): number {
  if (!sameDirection) return 1;
  return Math.min(8, current * 2);
}
