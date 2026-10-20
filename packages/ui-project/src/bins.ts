import { TICKS_PER_SECOND } from "@videocut/model";
import type { BinItemKind, BinNode, MediaRef, Project } from "@videocut/model";

/** Shells pass the real media importer here. The panel does not probe files. */
export interface MediaImporter {
  importPaths(paths: readonly string[]): Promise<readonly MediaRef[]>;
}

export interface CreateBinOptions {
  name?: string;
  id?: string;
}

export interface BinLocation {
  node: BinNode;
  parentId: string | null;
}

export class BinEditError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BinEditError";
  }
}

export function createId(prefix: string): string {
  return `${prefix}-${crypto.randomUUID()}`;
}

export function binKindLabel(kind: BinItemKind): string {
  switch (kind) {
    case "bin":
      return "Bin";
    case "clip":
      return "Clip";
    case "sequence":
      return "Sequence";
    default: {
      const unknown: never = kind;
      return unknown;
    }
  }
}

export function mediaFromPath(path: string, id?: string): MediaRef {
  const trimmed = path.trim();
  const base = trimmed.split(/[/\\]/).pop() || trimmed || "media";
  return {
    id: id ?? createId("media"),
    name: base,
    path: trimmed,
    durationTicks: 0,
  };
}

export function formatMediaDuration(ticks: number): string | null {
  if (!Number.isFinite(ticks) || ticks <= 0) return null;
  const totalSeconds = Math.round(ticks / TICKS_PER_SECOND);
  const seconds = totalSeconds % 60;
  const minutes = Math.floor(totalSeconds / 60) % 60;
  const hours = Math.floor(totalSeconds / 3600);
  const pad = (value: number) => String(value).padStart(2, "0");
  if (hours > 0) return `${hours}:${pad(minutes)}:${pad(seconds)}`;
  return `${minutes}:${pad(seconds)}`;
}

export function locateBinNode(
  nodes: readonly BinNode[],
  id: string,
  parentId: string | null = null,
): BinLocation | null {
  for (const node of nodes) {
    if (node.id === id) return { node, parentId };
    if (node.kind === "bin" && node.children?.length) {
      const found = locateBinNode(node.children, id, node.id);
      if (found) return found;
    }
  }
  return null;
}

export function binPath(nodes: readonly BinNode[], binId: string | null): BinNode[] {
  if (binId === null) return [];
  const path: BinNode[] = [];
  const walk = (level: readonly BinNode[]): boolean => {
    for (const node of level) {
      if (node.kind !== "bin") continue;
      if (node.id === binId) {
        path.push(node);
        return true;
      }
      if (node.children?.length && walk(node.children)) {
        path.push(node);
        return true;
      }
    }
    return false;
  };
  walk(nodes);
  return path.reverse();
}

export function visibleBinNodes(project: Project, binId: string | null, query: string): BinNode[] {
  let children: readonly BinNode[];
  if (binId === null) {
    children = project.bins;
  } else {
    const located = locateBinNode(project.bins, binId);
    if (!located || located.node.kind !== "bin") return [];
    children = located.node.children ?? [];
  }
  const needle = query.trim().toLowerCase();
  if (!needle) return [...children];
  return flattenBins(children).filter((node) => node.name.toLowerCase().includes(needle));
}

export function createBin(
  project: Project,
  parentId: string | null,
  options: CreateBinOptions = {},
): Project {
  assertBinParent(project, parentId);
  const requested = options.name?.trim() ?? "";
  const name = requested || uniqueName(siblingNames(project, parentId), "Bin");
  const bin: BinNode = {
    id: options.id ?? createId("bin"),
    name,
    kind: "bin",
  };
  return { ...project, bins: insertChild(project.bins, parentId, bin) };
}

export function renameBinItem(project: Project, nodeId: string, name: string): Project {
  const trimmed = name.trim();
  if (!trimmed) return project;
  const located = locateBinNode(project.bins, nodeId);
  if (!located) throw new BinEditError(`Bin item not found: ${nodeId}`);

  let bins = project.bins;
  let media = project.media;
  let sequences = project.sequences;

  switch (located.node.kind) {
    case "bin":
      bins = mapBinTree(bins, (node) =>
        node.id === nodeId && node.name !== trimmed ? { ...node, name: trimmed } : node,
      );
      break;
    case "clip": {
      const mediaId = located.node.mediaId;
      if (mediaId) {
        media = mapItems(media, (item) =>
          item.id === mediaId && item.name !== trimmed ? { ...item, name: trimmed } : item,
        );
        bins = mapBinTree(bins, (node) =>
          node.kind === "clip" && node.mediaId === mediaId && node.name !== trimmed
            ? { ...node, name: trimmed }
            : node,
        );
      } else {
        bins = mapBinTree(bins, (node) =>
          node.id === nodeId && node.name !== trimmed ? { ...node, name: trimmed } : node,
        );
      }
      break;
    }
    case "sequence": {
      const sequenceId = located.node.sequenceId;
      if (sequenceId) {
        sequences = mapItems(sequences, (item) =>
          item.id === sequenceId && item.name !== trimmed ? { ...item, name: trimmed } : item,
        );
        bins = mapBinTree(bins, (node) =>
          node.kind === "sequence" && node.sequenceId === sequenceId && node.name !== trimmed
            ? { ...node, name: trimmed }
            : node,
        );
      } else {
        bins = mapBinTree(bins, (node) =>
          node.id === nodeId && node.name !== trimmed ? { ...node, name: trimmed } : node,
        );
      }
      break;
    }
    default: {
      const unknown: never = located.node.kind;
      throw new BinEditError(`Unknown bin item: ${String(unknown)}`);
    }
  }

  if (bins === project.bins && media === project.media && sequences === project.sequences) {
    return project;
  }
  return { ...project, bins, media, sequences };
}

export function addImportedMedia(
  project: Project,
  parentId: string | null,
  imported: readonly MediaRef[],
): Project {
  if (imported.length === 0) return project;
  assertBinParent(project, parentId);

  let media = project.media;
  let bins = project.bins;
  let changed = false;

  for (const item of imported) {
    if (!item.id.trim()) throw new BinEditError("Imported media is missing an id");
    const existing =
      media.find((entry) => entry.id === item.id) ??
      (item.path ? media.find((entry) => entry.path === item.path) : undefined);
    const mediaId = existing?.id ?? item.id;
    if (!existing) {
      media = [...media, { ...item }];
      changed = true;
    }
    if (directChildren(bins, parentId).some((node) => node.kind === "clip" && node.mediaId === mediaId)) {
      continue;
    }
    const clip: BinNode = {
      id: createId("bin"),
      name: existing?.name ?? item.name,
      kind: "clip",
      mediaId,
    };
    bins = insertChild(bins, parentId, clip);
    changed = true;
  }

  if (!changed) return project;
  return { ...project, media, bins };
}

function assertBinParent(project: Project, parentId: string | null): void {
  if (parentId === null) return;
  const located = locateBinNode(project.bins, parentId);
  if (!located) throw new BinEditError(`Bin not found: ${parentId}`);
  if (located.node.kind !== "bin") throw new BinEditError("Items can only be added to a bin");
}

function siblingNames(project: Project, parentId: string | null): string[] {
  return directChildren(project.bins, parentId).map((node) => node.name);
}

function directChildren(nodes: readonly BinNode[], parentId: string | null): readonly BinNode[] {
  if (parentId === null) return nodes;
  return locateBinNode(nodes, parentId)?.node.children ?? [];
}

function uniqueName(existing: readonly string[], base: string): string {
  const taken = new Set(existing);
  if (!taken.has(base)) return base;
  let index = 2;
  while (taken.has(`${base} ${index}`)) index += 1;
  return `${base} ${index}`;
}

function flattenBins(nodes: readonly BinNode[]): BinNode[] {
  const flat: BinNode[] = [];
  for (const node of nodes) {
    flat.push(node);
    if (node.kind === "bin" && node.children?.length) {
      flat.push(...flattenBins(node.children));
    }
  }
  return flat;
}

function insertChild(nodes: readonly BinNode[], parentId: string | null, child: BinNode): BinNode[] {
  if (parentId === null) return [...nodes, child];
  let changed = false;
  const next = nodes.map((node) => {
    if (node.id === parentId) {
      changed = true;
      return { ...node, children: [...(node.children ?? []), child] };
    }
    if (node.kind !== "bin" || !node.children?.length) return node;
    const children = insertChild(node.children, parentId, child);
    if (children === node.children) return node;
    changed = true;
    return { ...node, children };
  });
  return changed ? next : (nodes as BinNode[]);
}

function mapBinTree(nodes: readonly BinNode[], visit: (node: BinNode) => BinNode): BinNode[] {
  let changed = false;
  const next = nodes.map((node) => {
    let updated = visit(node);
    if (updated.kind === "bin" && updated.children?.length) {
      const children = mapBinTree(updated.children, visit);
      if (children !== updated.children) updated = { ...updated, children };
    }
    if (updated !== node) changed = true;
    return updated;
  });
  return changed ? next : (nodes as BinNode[]);
}

function mapItems<T>(items: readonly T[], visit: (item: T) => T): T[] {
  let changed = false;
  const next = items.map((item) => {
    const updated = visit(item);
    if (updated !== item) changed = true;
    return updated;
  });
  return changed ? next : (items as T[]);
}
