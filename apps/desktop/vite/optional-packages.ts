import fs from "node:fs";
import path from "node:path";
import type { Plugin } from "vite";

export const OPTIONAL_UI_PACKAGES = [
  "@videocut/ui-timeline",
  "@videocut/ui-effects",
  "@videocut/ui-color",
  "@videocut/ui-audio",
  "@videocut/ui-graphics",
  "@videocut/ui-project",
  "@videocut/ui-export",
] as const;

export type OptionalUiPackage = (typeof OPTIONAL_UI_PACKAGES)[number];

interface PackageJson {
  main?: string;
  module?: string;
}

export function resolveWorkspaceEntry(workspaceRoot: string, specifier: string): string | null {
  if (!specifier.startsWith("@videocut/")) return null;
  const folder = specifier.slice("@videocut/".length);
  const packageDir = path.join(workspaceRoot, "packages", folder);
  const manifestPath = path.join(packageDir, "package.json");
  if (!fs.existsSync(manifestPath)) return null;
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8")) as PackageJson;
  const entry = manifest.module ?? manifest.main;
  if (!entry) return null;
  const resolved = path.resolve(packageDir, entry);
  return fs.existsSync(resolved) ? resolved : null;
}

export function optionalUiPlugin(workspaceRoot: string): Plugin {
  const names = new Set<string>(OPTIONAL_UI_PACKAGES);
  return {
    name: "videocut-optional-ui",
    enforce: "pre",
    resolveId(id) {
      if (!names.has(id)) return null;
      return resolveWorkspaceEntry(workspaceRoot, id) ?? `\0videocut-optional:${id}`;
    },
    load(id) {
      if (!id.startsWith("\0videocut-optional:")) return null;
      return "export default null;\nexport const available = false;\n";
    },
  };
}
