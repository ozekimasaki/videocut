import { useState } from "react";
import type { Sequence } from "@videocut/model";
import { commands } from "../host/edits";
import { AUDIO_EXPORTS, COLOR_EXPORTS, EFFECTS_EXPORTS, GRAPHICS_EXPORTS, resolvePanel } from "../host/panels";
import type { PanelHostProps } from "../host/panel-props";
import { findClip } from "../host/playhead";
import type { Editor } from "../host/use-editor";
import { labelColor } from "../ui/colors";
import { audioModule, colorModule, effectsModule, graphicsModule } from "../ui/modules";

const TABS = [
  { id: "effects", label: "エフェクト" },
  { id: "color", label: "カラー" },
  { id: "audio", label: "オーディオ" },
  { id: "graphics", label: "グラフィック" },
] as const;

type TabId = (typeof TABS)[number]["id"];

export function SidePanels({ editor }: { editor: Editor }) {
  const [tab, setTab] = useState<TabId>("effects");
  const sequence = editor.project.sequences.find((item) => item.id === editor.session.activeSequenceId) ?? null;
  const host: PanelHostProps = {
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
  return (
    <section className="panel side" data-testid="side-panels">
      <div className="tabs" role="tablist">
        {TABS.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={tab === item.id}
            data-testid={`tab-${item.id}`}
            className={tab === item.id ? "tabs__button is-active" : "tabs__button"}
            onClick={() => setTab(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>
      <div className="side__body" role="tabpanel">
        {tab === "effects" ? <Effects host={host} module={effectsModule} /> : null}
        {tab === "color" ? <Color host={host} module={colorModule} /> : null}
        {tab === "audio" ? <Audio host={host} module={audioModule} sequence={sequence} /> : null}
        {tab === "graphics" ? <Graphics host={host} module={graphicsModule} sequence={sequence} /> : null}
      </div>
    </section>
  );
}

function Effects({ host, module }: { host: PanelHostProps; module: object }) {
  const External = resolvePanel(module, EFFECTS_EXPORTS);
  if (External) return <External {...host} />;
  const sequence = host.sequence;
  const clip = sequence && host.selectedClipIds[0] ? findClip(sequence, host.selectedClipIds[0])?.clip : null;
  return (
    <div className="fallback" data-testid="effects-fallback">
      <h2>エフェクトコントロール</h2>
      {clip ? <p>{clip.name}</p> : <p className="empty">クリップを選択</p>}
      <ul>
        {(clip?.effects ?? []).map((effect) => (
          <li key={effect.id}>
            <label>
              <input
                type="checkbox"
                checked={effect.enabled}
                onChange={(event) => {
                  if (!host.sequenceId || !clip) return;
                  host.execute(commands.toggleEffect(host.sequenceId, clip.id, effect.id, event.target.checked));
                }}
              />
              {effect.effectId}
            </label>
          </li>
        ))}
      </ul>
      {clip && (clip.effects ?? []).length === 0 ? <p className="empty">エフェクトなし</p> : null}
    </div>
  );
}

function Color({ host, module }: { host: PanelHostProps; module: object }) {
  const External = resolvePanel(module, COLOR_EXPORTS);
  if (External) return <External {...host} />;
  const sequence = host.sequence;
  const clip = sequence && host.selectedClipIds[0] ? findClip(sequence, host.selectedClipIds[0])?.clip : null;
  return (
    <div className="fallback" data-testid="color-fallback">
      <h2>カラー</h2>
      {clip ? (
        <div className="swatch" style={{ background: labelColor(clip.labelColor) }}>
          <span>{clip.name}</span>
        </div>
      ) : (
        <p className="empty">クリップを選択</p>
      )}
      <p className="hint">ホイールとスコープはカラーパネルが接続されると表示されます。</p>
    </div>
  );
}

function Audio({ host, module, sequence }: { host: PanelHostProps; module: object; sequence: Sequence | null }) {
  const External = resolvePanel(module, AUDIO_EXPORTS);
  if (External) return <External {...host} />;
  return (
    <div className="fallback" data-testid="audio-fallback">
      <h2>オーディオ</h2>
      {(sequence?.audioTracks ?? []).map((track) => (
        <div className="mixer-row" key={track.id}>
          <span>A{track.index + 1}</span>
          <button
            type="button"
            aria-pressed={track.muted}
            onClick={() => host.sequenceId && host.execute(commands.patchTrack(host.sequenceId, track.id, { muted: !track.muted }))}
          >
            {track.muted ? "ミュート中" : "ミュート"}
          </button>
        </div>
      ))}
      {(sequence?.audioTracks ?? []).length === 0 ? <p className="empty">オーディオトラックなし</p> : null}
    </div>
  );
}

function Graphics({ host, module, sequence }: { host: PanelHostProps; module: object; sequence: Sequence | null }) {
  const External = resolvePanel(module, GRAPHICS_EXPORTS);
  if (External) return <External {...host} />;
  const titles = [...(sequence?.videoTracks ?? [])].flatMap((track) => track.clips.filter((clip) => clip.kind === "title"));
  return (
    <div className="fallback" data-testid="graphics-fallback">
      <h2>グラフィック</h2>
      {titles.map((clip) => (
        <p key={clip.id}>{clip.name}</p>
      ))}
      <ul>
        {(sequence?.captions ?? []).map((caption) => (
          <li key={caption.id}>
            <input
              aria-label="キャプション"
              defaultValue={caption.text}
              onBlur={(event) => {
                if (!host.sequenceId || event.target.value === caption.text) return;
                host.execute(commands.caption(host.sequenceId, caption.id, event.target.value));
              }}
            />
          </li>
        ))}
      </ul>
      {titles.length === 0 && (sequence?.captions ?? []).length === 0 ? <p className="empty">タイトルもキャプションもありません</p> : null}
    </div>
  );
}
