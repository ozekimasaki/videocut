import { useEffect, useMemo, useState } from "react";
import type { BinNode } from "@videocut/model";
import { commands } from "../host/edits";
import { PROJECT_EXPORTS, resolvePanel } from "../host/panels";
import type { PanelHostProps } from "../host/panel-props";
import type { Editor } from "../host/use-editor";
import { projectModule } from "../ui/modules";

export function ProjectBin({ editor }: { editor: Editor }) {
  const host = hostProps(editor);
  const External = resolvePanel(projectModule, PROJECT_EXPORTS);
  if (External) return <External {...host} />;
  return <FallbackBin editor={editor} />;
}

function FallbackBin({ editor }: { editor: Editor }) {
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<"list" | "icon">("list");
  const [renaming, setRenaming] = useState<string | null>(null);
  const [name, setName] = useState(editor.project.name);
  useEffect(() => setName(editor.project.name), [editor.project.name]);
  const items = useMemo(() => flatten(editor.project.bins, query.trim().toLowerCase()), [editor.project.bins, query]);
  return (
    <section className="panel bin" data-testid="project-bin">
      <header className="panel__title">
        <span>プロジェクト</span>
        <div className="panel__actions">
          <button type="button" aria-pressed={mode === "list"} onClick={() => setMode("list")}>
            リスト
          </button>
          <button type="button" aria-pressed={mode === "icon"} onClick={() => setMode("icon")}>
            アイコン
          </button>
        </div>
      </header>
      <div className="bin__search">
        <input
          value={query}
          placeholder="検索"
          aria-label="プロジェクトを検索"
          onChange={(event) => setQuery(event.target.value)}
        />
        <button type="button" onClick={() => void editor.importMedia()}>
          読み込み
        </button>
      </div>
      <div className="bin__name">
        <input
          aria-label="プロジェクト名"
          value={name}
          onChange={(event) => setName(event.target.value)}
          onBlur={() => editor.execute(commands.renameProject(name))}
          onKeyDown={(event) => {
            if (event.key === "Enter") event.currentTarget.blur();
          }}
        />
      </div>
      {items.length === 0 ? <p className="empty">項目がありません</p> : null}
      <ul className={mode === "icon" ? "bin__items bin__items--icon" : "bin__items"}>
        {items.map((item) => (
          <li key={item.id}>
            {renaming === item.id ? (
              <input
                autoFocus
                defaultValue={item.name}
                aria-label="名前"
                onBlur={(event) => {
                  editor.execute(commands.renameBin(item.id, event.target.value));
                  setRenaming(null);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter") event.currentTarget.blur();
                  if (event.key === "Escape") setRenaming(null);
                }}
              />
            ) : (
              <button
                type="button"
                className="bin__item"
                onClick={() => openItem(editor, item)}
              >
                <i data-kind={item.kind} />
                <span onDoubleClick={(event) => {
                  event.stopPropagation();
                  setRenaming(item.id);
                }}>{item.name}</span>
              </button>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

function openItem(editor: Editor, item: FlatItem): void {
  if (item.kind === "sequence" && item.sequenceId) editor.activateSequence(item.sequenceId);
  if (item.kind === "clip" && item.mediaId) editor.openSourceMedia(item.mediaId);
}

interface FlatItem {
  id: string;
  name: string;
  kind: BinNode["kind"];
  mediaId?: string;
  sequenceId?: string;
}

function flatten(nodes: BinNode[], query: string): FlatItem[] {
  const items: FlatItem[] = [];
  const visit = (node: BinNode) => {
    const matches = query.length === 0 || node.name.toLowerCase().includes(query);
    if (matches && node.kind !== "bin") {
      items.push({
        id: node.id,
        name: node.name,
        kind: node.kind,
        ...(node.mediaId ? { mediaId: node.mediaId } : {}),
        ...(node.sequenceId ? { sequenceId: node.sequenceId } : {}),
      });
    }
    if (node.kind === "bin" && matches && query.length === 0) {
      items.push({ id: node.id, name: node.name, kind: node.kind });
    }
    for (const child of node.children ?? []) visit(child);
  };
  for (const node of nodes) visit(node);
  return items;
}

function hostProps(editor: Editor): PanelHostProps {
  const sequence = editor.project.sequences.find((item) => item.id === editor.session.activeSequenceId) ?? null;
  return {
    project: editor.project,
    sequence,
    sequenceId: editor.session.activeSequenceId,
    tool: editor.session.tool,
    playheadTicks: editor.session.playheadTicks,
    selectedClipIds: editor.session.selectedClipIds,
    markInTicks: editor.session.programMarkIn,
    markOutTicks: editor.session.programMarkOut,
    execute: editor.execute,
    setPlayhead: editor.setPlayhead,
    setSelectedClipIds: editor.setSelected,
  };
}
