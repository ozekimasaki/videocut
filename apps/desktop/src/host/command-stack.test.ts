import type { Project } from "@videocut/model";
import { describe, expect, it } from "vitest";
import { CommandStack } from "./command-stack";

function project(name: string): Project {
  return { id: "proj-1", name, bins: [], sequences: [], media: [] };
}

describe("CommandStack", () => {
  it("undoes and redoes a named edit", () => {
    const stack = new CommandStack(project("無題"));
    const changed = stack.execute({
      label: "名前の変更",
      apply: (current) => ({ ...current, name: "本編" }),
    });
    expect(changed).toBe(true);
    expect(stack.project.name).toBe("本編");
    expect(stack.lastUndoLabel).toBe("名前の変更");
    expect(stack.dirty).toBe(true);
    expect(stack.undo()).toBe(true);
    expect(stack.project.name).toBe("無題");
    expect(stack.dirty).toBe(false);
    expect(stack.redo()).toBe(true);
    expect(stack.project.name).toBe("本編");
    stack.markSaved();
    expect(stack.dirty).toBe(false);
  });

  it("does not record a command that leaves the project unchanged", () => {
    const stack = new CommandStack(project("無題"));
    expect(stack.execute({ label: "空", apply: (current) => current })).toBe(false);
    expect(stack.canUndo).toBe(false);
  });

  it("clears the redo stack after a new edit", () => {
    const stack = new CommandStack(project("A"));
    stack.execute({ label: "B", apply: (current) => ({ ...current, name: "B" }) });
    stack.undo();
    stack.execute({ label: "C", apply: (current) => ({ ...current, name: "C" }) });
    expect(stack.canRedo).toBe(false);
    expect(stack.project.name).toBe("C");
  });

  it("keeps the previous project intact when a command mutates its draft", () => {
    const stack = new CommandStack(project("無題"));
    stack.execute({
      label: "破壊的",
      apply: (current) => {
        current.name = "変更";
        return current;
      },
    });
    stack.undo();
    expect(stack.project.name).toBe("無題");
  });
});
