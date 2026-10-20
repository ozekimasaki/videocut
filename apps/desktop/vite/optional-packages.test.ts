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

  it("resolves the feature panels that ship in the workspace", () => {
    expect(resolveWorkspaceEntry(workspaceRoot, "@videocut/ui-timeline")).toMatch(/packages\/ui-timeline\/src\/index\.ts$/);
    expect(resolveWorkspaceEntry(workspaceRoot, "@videocut/ui-effects")).toMatch(/packages\/ui-effects\/src\/index\.ts$/);
    expect(resolveWorkspaceEntry(workspaceRoot, "@videocut/ui-color")).toMatch(/packages\/ui-color\/src\/index\.ts$/);
    expect(resolveWorkspaceEntry(workspaceRoot, "@videocut/ui-audio")).toMatch(/packages\/ui-audio\/src\/index\.ts$/);
    expect(resolveWorkspaceEntry(workspaceRoot, "@videocut/ui-graphics")).toMatch(/packages\/ui-graphics\/src\/index\.ts$/);
    expect(resolveWorkspaceEntry(workspaceRoot, "@videocut/ui-project")).toMatch(/packages\/ui-project\/src\/index\.ts$/);
    expect(resolveWorkspaceEntry(workspaceRoot, "@videocut/ui-export")).toMatch(/packages\/ui-export\/src\/index\.ts$/);
  });

  it("returns null when a package is not in the workspace", () => {
    expect(resolveWorkspaceEntry(workspaceRoot, "@videocut/ui-missing")).toBeNull();
  });
});
