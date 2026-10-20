import { useEffect, useId, useMemo, useRef, useState } from "react";
import type { DragEvent, KeyboardEvent } from "react";
import type { BinItemKind, BinNode, Project } from "@videocut/model";
import {
  addImportedMedia,
  binKindLabel,
  binPath,
  createBin,
  createId,
  formatMediaDuration,
  locateBinNode,
  mediaFromPath,
  renameBinItem,
  visibleBinNodes,
} from "./bins";
import type { MediaImporter } from "./bins";
import { ClipIcon, FolderIcon, GridIcon, ListIcon, SequenceIcon } from "./icons";
import "./ProjectPanel.css";

const IMPORT_ACCEPT = ".mp4,.mov,.mxf,.wav,.aac,.mp3,.png,.jpg,.jpeg";

export type BinView = "list" | "icon";

export interface ProjectPanelProps {
  project: Project;
  onProjectChange: (project: Project) => void;
  importer?: MediaImporter;
  /** Native file dialog. Used when a file input would not expose local paths. */
  onPickFiles?: () => Promise<readonly string[]>;
  onOpenSequence?: (sequenceId: string) => void;
  onOpenClip?: (mediaId: string) => void;
  className?: string;
}

export function ProjectPanel({
  project,
  onProjectChange,
  importer,
  onPickFiles,
  onOpenSequence,
  onOpenClip,
  className,
}: ProjectPanelProps) {
  const [query, setQuery] = useState("");
  const [view, setView] = useState<BinView>("list");
  const [currentBinId, setCurrentBinId] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [draftName, setDraftName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);
  const projectRef = useRef(project);
  const renamingRef = useRef<string | null>(null);
  const draftRef = useRef("");
  const importLock = useRef(false);
  const searchId = useId();
  projectRef.current = project;

  const locatedBin = currentBinId ? locateBinNode(project.bins, currentBinId) : null;
  const activeBinId = locatedBin?.node.kind === "bin" ? currentBinId : null;
  const binMissing = currentBinId !== null && locatedBin?.node.kind !== "bin";
  const items = useMemo(
    () => visibleBinNodes(project, activeBinId, query),
    [project, activeBinId, query],
  );
  const crumbs = binPath(project.bins, activeBinId);
  const selected = selectedId ? items.find((item) => item.id === selectedId) ?? null : null;

  useEffect(() => {
    if (binMissing) setCurrentBinId(null);
  }, [binMissing]);

  useEffect(() => {
    if (selectedId && !locateBinNode(project.bins, selectedId)) {
      setSelectedId(null);
      if (renamingRef.current === selectedId) renamingRef.current = null;
      setRenamingId(null);
    }
  }, [project, selectedId]);

  function finishRename(id: string): Project | null {
    if (renamingRef.current !== id) return null;
    renamingRef.current = null;
    setRenamingId(null);
    try {
      const next = renameBinItem(projectRef.current, id, draftRef.current);
      projectRef.current = next;
      setError(null);
      return next;
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Rename failed");
      return null;
    }
  }

  function commitRename(id: string) {
    if (renamingRef.current !== id) return;
    const current = projectRef.current;
    const next = finishRename(id);
    if (next && next !== current) onProjectChange(next);
  }

  function cancelRename(id: string) {
    if (renamingRef.current !== id) return;
    renamingRef.current = null;
    setRenamingId(null);
  }

  function startRename(node: BinNode) {
    if (renamingRef.current === node.id) return;
    if (renamingRef.current) commitRename(renamingRef.current);
    renamingRef.current = node.id;
    draftRef.current = node.name;
    setSelectedId(node.id);
    setRenamingId(node.id);
    setDraftName(node.name);
  }

  function handleNewBin() {
    let base = projectRef.current;
    if (renamingRef.current) {
      const renamed = finishRename(renamingRef.current);
      if (!renamed) return;
      base = renamed;
    }
    setQuery("");
    try {
      const id = createId("bin");
      const next = createBin(base, activeBinId, { id });
      const created = locateBinNode(next.bins, id);
      const name = created?.node.name ?? "Bin";
      projectRef.current = next;
      renamingRef.current = id;
      draftRef.current = name;
      onProjectChange(next);
      setSelectedId(id);
      setDraftName(name);
      setRenamingId(id);
      setError(null);
    } catch (caught) {
      projectRef.current = base;
      if (base !== project) onProjectChange(base);
      setError(caught instanceof Error ? caught.message : "Could not create a bin");
    }
  }

  async function importPaths(paths: readonly string[]) {
    if (paths.length === 0 || importLock.current) return;
    const parentId = activeBinId;
    importLock.current = true;
    setImporting(true);
    try {
      const imported = importer
        ? await importer.importPaths(paths)
        : paths.map((path) => mediaFromPath(path));
      if (imported.length === 0) {
        setError("Nothing was imported.");
        return;
      }
      onProjectChange(addImportedMedia(projectRef.current, parentId, imported));
      setQuery("");
      setError(null);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Import failed");
    } finally {
      importLock.current = false;
      setImporting(false);
    }
  }

  async function handleImportClick() {
    if (onPickFiles) {
      try {
        await importPaths(await onPickFiles());
      } catch (caught) {
        setError(caught instanceof Error ? caught.message : "Import failed");
      }
      return;
    }
    fileRef.current?.click();
  }

  function activate(node: BinNode) {
    switch (node.kind) {
      case "bin":
        if (renamingRef.current) commitRename(renamingRef.current);
        setCurrentBinId(node.id);
        setQuery("");
        setSelectedId(null);
        return;
      case "sequence":
        if (node.sequenceId) onOpenSequence?.(node.sequenceId);
        return;
      case "clip":
        if (node.mediaId) onOpenClip?.(node.mediaId);
        return;
      default: {
        const unknown: never = node.kind;
        return unknown;
      }
    }
  }

  function moveSelection(delta: number) {
    if (items.length === 0) return;
    const index = items.findIndex((item) => item.id === selectedId);
    const nextIndex =
      index < 0
        ? delta > 0
          ? 0
          : items.length - 1
        : Math.min(items.length - 1, Math.max(0, index + delta));
    const next = items[nextIndex];
    if (next) setSelectedId(next.id);
  }

  function onListKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.target instanceof HTMLInputElement) return;
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        moveSelection(1);
        return;
      case "ArrowUp":
        event.preventDefault();
        moveSelection(-1);
        return;
      case "Home":
        event.preventDefault();
        if (items[0]) setSelectedId(items[0].id);
        return;
      case "End": {
        event.preventDefault();
        const last = items.at(-1);
        if (last) setSelectedId(last.id);
        return;
      }
      case "Enter":
        if (selected) {
          event.preventDefault();
          activate(selected);
        }
        return;
      case "F2":
        if (selected) {
          event.preventDefault();
          startRename(selected);
        }
        return;
      case "Backspace":
        if (!query && crumbs.length > 0) {
          event.preventDefault();
          setCurrentBinId(crumbs.at(-2)?.id ?? null);
          setSelectedId(null);
        }
        return;
      default:
        return;
    }
  }

  function onDrop(event: DragEvent<HTMLElement>) {
    event.preventDefault();
    setDragOver(false);
    const files = event.dataTransfer.files;
    if (files.length === 0) return;
    void importPaths(Array.from(files, filePath));
  }

  const rootClass = ["vc-project", dragOver ? "vc-project--drop" : "", className ?? ""]
    .filter(Boolean)
    .join(" ");

  return (
    <section
      aria-label="Project"
      className={rootClass}
      onDragOver={(event) => {
        event.preventDefault();
        setDragOver(true);
      }}
      onDragLeave={() => setDragOver(false)}
      onDrop={onDrop}
    >
      <div className="vc-project__toolbar">
        <input
          id={searchId}
          className="vc-project__search"
          type="search"
          value={query}
          placeholder="Search"
          aria-label="Search"
          onChange={(event) => setQuery(event.target.value)}
        />
        <div className="vc-project__views" role="group" aria-label="View">
          <button
            type="button"
            aria-label="List"
            aria-pressed={view === "list"}
            onClick={() => setView("list")}
          >
            <ListIcon />
          </button>
          <button
            type="button"
            aria-label="Icons"
            aria-pressed={view === "icon"}
            onClick={() => setView("icon")}
          >
            <GridIcon />
          </button>
        </div>
      </div>
      <div className="vc-project__actions">
        <button type="button" onClick={handleNewBin}>
          New Bin
        </button>
        <button type="button" onClick={() => void handleImportClick()} disabled={importing}>
          {importing ? "Importing…" : "Import"}
        </button>
        <button type="button" onClick={() => selected && startRename(selected)} disabled={!selected}>
          Rename
        </button>
        <input
          ref={fileRef}
          className="vc-project__file"
          type="file"
          multiple
          accept={IMPORT_ACCEPT}
          tabIndex={-1}
          aria-hidden="true"
          onChange={(event) => {
            const files = event.target.files;
            if (files && files.length > 0) void importPaths(Array.from(files, filePath));
            event.target.value = "";
          }}
        />
      </div>
      <nav className="vc-project__crumbs" aria-label="Bin path">
        <button type="button" onClick={() => setCurrentBinId(null)}>
          {project.name || "Project"}
        </button>
        {crumbs.map((bin) => (
          <span key={bin.id}>
            <span aria-hidden="true">/</span>{" "}
            <button type="button" onClick={() => setCurrentBinId(bin.id)}>
              {bin.name}
            </button>
          </span>
        ))}
      </nav>
      {error ? (
        <p className="vc-project__error" role="alert">
          {error}
        </p>
      ) : null}
      <div
        className={view === "icon" ? "vc-project__list vc-project__list--icon" : "vc-project__list"}
        role="listbox"
        aria-label="Bin items"
        tabIndex={0}
        onKeyDown={onListKeyDown}
      >
        {items.length === 0 ? (
          <p className="vc-project__empty">{query.trim() ? "No items match" : "This bin is empty"}</p>
        ) : (
          items.map((node) => (
            <BinItem
              key={node.id}
              node={node}
              view={view}
              selected={node.id === selectedId}
              renaming={node.id === renamingId}
              draftName={draftName}
              duration={durationLabel(project, node)}
              onDraftName={(name) => {
                draftRef.current = name;
                setDraftName(name);
              }}
              onSelect={() => setSelectedId(node.id)}
              onOpen={() => activate(node)}
              onCommit={() => commitRename(node.id)}
              onCancel={() => cancelRename(node.id)}
            />
          ))
        )}
      </div>
    </section>
  );
}

function durationLabel(project: Project, node: BinNode): string | null {
  if (node.kind !== "clip" || !node.mediaId) return null;
  const media = project.media.find((item) => item.id === node.mediaId);
  if (!media) return null;
  return formatMediaDuration(media.durationTicks);
}

function filePath(file: File): string {
  const withPath = file as File & { path?: string };
  return typeof withPath.path === "string" && withPath.path.length > 0 ? withPath.path : file.name;
}

interface BinItemProps {
  node: BinNode;
  view: BinView;
  selected: boolean;
  renaming: boolean;
  draftName: string;
  duration: string | null;
  onDraftName: (name: string) => void;
  onSelect: () => void;
  onOpen: () => void;
  onCommit: () => void;
  onCancel: () => void;
}

function BinItem({
  node,
  view,
  selected,
  renaming,
  draftName,
  duration,
  onDraftName,
  onSelect,
  onOpen,
  onCommit,
  onCancel,
}: BinItemProps) {
  const meta = duration ? `${binKindLabel(node.kind)} ${duration}` : binKindLabel(node.kind);
  return (
    <div
      role="option"
      aria-selected={selected}
      aria-label={view === "icon" ? `${node.name}, ${meta}` : undefined}
      className="vc-project__item"
      onClick={onSelect}
      onDoubleClick={onOpen}
    >
      <KindIcon kind={node.kind} />
      {renaming ? (
        <input
          className="vc-project__rename"
          aria-label={`Rename ${node.name}`}
          value={draftName}
          autoFocus
          onChange={(event) => onDraftName(event.target.value)}
          onClick={(event) => event.stopPropagation()}
          onDoubleClick={(event) => event.stopPropagation()}
          onBlur={onCommit}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              onCommit();
            } else if (event.key === "Escape") {
              event.preventDefault();
              onCancel();
            }
            event.stopPropagation();
          }}
        />
      ) : (
        <span className="vc-project__name">{node.name}</span>
      )}
      <span className="vc-project__meta" aria-hidden={view === "icon" ? true : undefined}>
        {meta}
      </span>
    </div>
  );
}

function KindIcon({ kind }: { kind: BinItemKind }) {
  switch (kind) {
    case "bin":
      return <FolderIcon />;
    case "clip":
      return <ClipIcon />;
    case "sequence":
      return <SequenceIcon />;
    default: {
      const unknown: never = kind;
      return unknown;
    }
  }
}
