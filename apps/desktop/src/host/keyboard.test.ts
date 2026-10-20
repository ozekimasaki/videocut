import { describe, expect, it } from "vitest";
import { actionFromKey, nextShuttleSpeed } from "./keyboard";

const plain = { ctrlKey: false, metaKey: false, shiftKey: false, altKey: false };

describe("actionFromKey", () => {
  it("maps the standard edit keys", () => {
    expect(actionFromKey({ ...plain, key: "v" })).toEqual({ type: "tool", tool: "select" });
    expect(actionFromKey({ ...plain, key: "A" })).toEqual({ type: "tool", tool: "trackSelectForward" });
    expect(actionFromKey({ ...plain, key: "a", shiftKey: true })).toEqual({ type: "tool", tool: "trackSelectBackward" });
    expect(actionFromKey({ ...plain, key: "b" })).toEqual({ type: "tool", tool: "ripple" });
    expect(actionFromKey({ ...plain, key: "n" })).toEqual({ type: "tool", tool: "rolling" });
    expect(actionFromKey({ ...plain, key: "c" })).toEqual({ type: "tool", tool: "razor" });
    expect(actionFromKey({ ...plain, key: "y" })).toEqual({ type: "tool", tool: "slip" });
    expect(actionFromKey({ ...plain, key: "u" })).toEqual({ type: "tool", tool: "slide" });
    expect(actionFromKey({ ...plain, key: "p" })).toEqual({ type: "tool", tool: "pen" });
    expect(actionFromKey({ ...plain, key: "q" })).toEqual({ type: "trim-previous" });
    expect(actionFromKey({ ...plain, key: "w" })).toEqual({ type: "trim-next" });
    expect(actionFromKey({ ...plain, key: "i" })).toEqual({ type: "mark-in" });
    expect(actionFromKey({ ...plain, key: "o" })).toEqual({ type: "mark-out" });
    expect(actionFromKey({ ...plain, key: "j" })).toEqual({ type: "shuttle-reverse" });
    expect(actionFromKey({ ...plain, key: "k" })).toEqual({ type: "shuttle-stop" });
    expect(actionFromKey({ ...plain, key: "l" })).toEqual({ type: "shuttle-forward" });
  });

  it("maps undo and ignores ctrl+k", () => {
    expect(actionFromKey({ ...plain, key: "z", ctrlKey: true })).toEqual({ type: "undo" });
    expect(actionFromKey({ ...plain, key: "z", metaKey: true, shiftKey: true })).toEqual({ type: "redo" });
    expect(actionFromKey({ ...plain, key: "y", ctrlKey: true })).toEqual({ type: "redo" });
    expect(actionFromKey({ ...plain, key: "k", ctrlKey: true })).toBeNull();
  });
});

describe("nextShuttleSpeed", () => {
  it("doubles until eight", () => {
    expect(nextShuttleSpeed(1, false)).toBe(1);
    expect(nextShuttleSpeed(1, true)).toBe(2);
    expect(nextShuttleSpeed(8, true)).toBe(8);
  });
});
