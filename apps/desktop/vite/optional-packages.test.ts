import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { resolveWorkspaceEntry } from "./optional-packages";

const workspaceRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");

describe("resolveWorkspaceEntry", () => {
  it("resolves a workspace package that exists", () => {
    const entry = resolveWorkspaceEntry(workspaceRoot, "@videocut/model");
    expect(entry).toMatch(/packages\/model\/src\/types\.ts$/);
  });

  it("returns null when the panel package is not in the workspace", () => {
    expect(resolveWorkspaceEntry(workspaceRoot, "@videocut/ui-timeline")).toBeNull();
    expect(resolveWorkspaceEntry(workspaceRoot, "@videocut/ui-export")).toBeNull();
  });
});
