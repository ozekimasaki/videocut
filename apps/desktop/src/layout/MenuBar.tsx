import { useEffect, useState } from "react";
import type { ExchangeFormat } from "../host/desktop-bridge";
import { commands } from "../host/edits";
import type { Editor } from "../host/use-editor";

interface Item {
  label: string;
  shortcut?: string;
  disabled?: boolean;
  onSelect: () => void;
}

interface Menu {
  id: string;
  label: string;
  items: Item[];
}

export function MenuBar({ editor, onExport }: { editor: Editor; onExport: () => void }) {
  const [open, setOpen] = useState<string | null>(null);
  useEffect(() => {
    if (!open) return;
    const close = () => setOpen(null);
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
  }, [open]);

  const menus: Menu[] = [
    {
      id: "file",
      label: "ファイル",
      items: [
        { label: "新規プロジェクト", onSelect: editor.newProject },
        { label: "開く", onSelect: () => void editor.open() },
        { label: "保存", shortcut: "Ctrl+S", onSelect: () => void editor.save(false) },
        { label: "名前を付けて保存", onSelect: () => void editor.save(true) },
        { label: ".prproj を開く", onSelect: () => editor.openPrproj() },
        { label: ".prproj を保存", onSelect: () => editor.savePrproj() },
        { label: "メディアを読み込み", onSelect: () => void editor.importMedia() },
        { label: "書き出し", onSelect: onExport },
        { label: "終了", onSelect: editor.quit },
      ],
    },
    {
      id: "exchange",
      label: "交換",
      items: exchangeItems(editor),
    },
    {
      id: "edit",
      label: "編集",
      items: [
        { label: "取り消し", shortcut: "Ctrl+Z", disabled: !editor.canUndo, onSelect: editor.undo },
        { label: "やり直し", shortcut: "Ctrl+Shift+Z", disabled: !editor.canRedo, onSelect: editor.redo },
        {
          label: "リップル削除",
          shortcut: "Delete",
          disabled: editor.session.selectedClipIds.length === 0,
          onSelect: editor.deleteSelection,
        },
      ],
    },
    {
      id: "clip",
      label: "クリップ",
      items: [
        { label: "レーザーで分割", onSelect: editor.razorAtPlayhead },
        { label: "ソースを上書き", onSelect: editor.overwriteSource },
      ],
    },
    {
      id: "marker",
      label: "マーカー",
      items: [
        {
          label: "マーカーを追加",
          onSelect: () => {
            const sequenceId = editor.session.activeSequenceId;
            if (!sequenceId) return;
            editor.execute(commands.marker(sequenceId, editor.session.playheadTicks));
          },
        },
        { label: "インを設定", shortcut: "I", onSelect: editor.markIn },
        { label: "アウトを設定", shortcut: "O", onSelect: editor.markOut },
        { label: "イン・アウトをクリア", onSelect: editor.clearMarks },
      ],
    },
    {
      id: "sequence",
      label: "シーケンス",
      items: [
        {
          label: "シーケンスを追加",
          onSelect: () => {
            const existing = new Set(editor.committed.sequences.map((sequence) => sequence.id));
            const count = editor.committed.sequences.length + 1;
            const next = editor.execute(commands.addSequence(`シーケンス ${String(count).padStart(2, "0")}`));
            const created = next?.sequences.find((sequence) => !existing.has(sequence.id));
            if (created) editor.activateSequence(created.id);
          },
        },
      ],
    },
  ];

  return (
    <header className="menu-bar" data-testid="menu-bar">
      <span className="menu-bar__brand">VideoCut</span>
      {menus.map((menu) => (
        <div className="menu" key={menu.id}>
          <button
            type="button"
            className={open === menu.id ? "menu__button is-open" : "menu__button"}
            aria-expanded={open === menu.id}
            data-testid={`menu-${menu.id}`}
            onPointerDown={(event) => event.stopPropagation()}
            onClick={() => setOpen((current) => (current === menu.id ? null : menu.id))}
          >
            {menu.label}
          </button>
          {open === menu.id ? (
            <div className="menu__list" role="menu" onPointerDown={(event) => event.stopPropagation()}>
              {menu.items.map((item) => (
                <button
                  key={item.label}
                  type="button"
                  role="menuitem"
                  className="menu__item"
                  disabled={item.disabled}
                  onClick={() => {
                    setOpen(null);
                    item.onSelect();
                  }}
                >
                  <span>{item.label}</span>
                  {item.shortcut ? <kbd>{item.shortcut}</kbd> : null}
                </button>
              ))}
            </div>
          ) : null}
        </div>
      ))}
    </header>
  );
}

function exchangeItems(editor: Editor): Item[] {
  const formats: ExchangeFormat[] = ["otio", "fcpxml", "edl"];
  return formats.flatMap((format) => {
    const label = formatLabel(format);
    return [
      { label: `${label} を開く`, onSelect: () => editor.openExchange(format) },
      { label: `${label} を書き出す`, onSelect: () => editor.saveExchange(format) },
    ];
  });
}

function formatLabel(format: ExchangeFormat): string {
  switch (format) {
    case "otio":
      return "OTIO";
    case "fcpxml":
      return "FCPXML";
    case "edl":
      return "EDL";
    default: {
      const unknown: never = format;
      return unknown;
    }
  }
}
