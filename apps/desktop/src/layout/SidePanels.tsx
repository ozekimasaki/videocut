import { useEffect, useMemo, useState } from "react";
import type { EffectInstance, Sequence } from "@videocut/model";
import {
  editClip,
  editSequence,
  projectCommand,
  replaceClipEffects,
  replaceTransition,
  transitionNearClip,
} from "../host/document";
import { commands } from "../host/edits";
import { findClip } from "../host/playhead";
import type { Editor } from "../host/use-editor";
import {
  AudioMixer,
  ColorPanel,
  EffectControls,
  GraphicsPanel,
  mixerStateFromSequence,
  stripViews,
  updateCaptionText,
  updateCaptionTiming,
  withLowerThirdText,
  withTitleText,
  type MixerState,
} from "../ui/modules";

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
  const selectedId = editor.session.selectedClipIds[0] ?? null;
  const hit = sequence && selectedId ? findClip(sequence, selectedId) : null;
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
        {tab === "effects" ? <Effects editor={editor} sequence={sequence} /> : null}
        {tab === "color" ? <Color editor={editor} sequence={sequence} /> : null}
        {tab === "audio" ? <Audio editor={editor} sequence={sequence} /> : null}
        {tab === "graphics" ? (
          <GraphicsPanel
            sequence={sequence}
            selectedClip={hit?.clip ?? null}
            onTitleTextChange={(clipId, text) => {
              if (!sequence) return;
              editor.execute(
                projectCommand("タイトル", (project) =>
                  editClip(project, sequence.id, clipId, (clip) => {
                    const edited = withTitleText(clip, text);
                    return edited.ok ? edited.value : null;
                  }),
                ),
              );
            }}
            onLowerThirdChange={(clipId, primary, secondary) => {
              if (!sequence) return;
              editor.execute(
                projectCommand("ローワーサード", (project) =>
                  editClip(project, sequence.id, clipId, (clip) => {
                    const edited = withLowerThirdText(clip, primary, secondary);
                    return edited.ok ? edited.value : null;
                  }),
                ),
              );
            }}
            onCaptionTextChange={(captionId, text) => {
              if (!sequence) return;
              editor.execute(
                projectCommand("字幕", (project) =>
                  editSequence(project, sequence.id, (current) => {
                    const edited = updateCaptionText(current, captionId, text);
                    return edited.ok ? edited.value : null;
                  }),
                ),
              );
            }}
            onCaptionTimingChange={(captionId, startTicks, endTicks) => {
              if (!sequence) return;
              editor.execute(
                projectCommand("字幕", (project) =>
                  editSequence(project, sequence.id, (current) => {
                    const edited = updateCaptionTiming(current, captionId, startTicks, endTicks);
                    return edited.ok ? edited.value : null;
                  }),
                ),
              );
            }}
          />
        ) : null}
      </div>
    </section>
  );
}

function Effects({ editor, sequence }: { editor: Editor; sequence: Sequence | null }) {
  const clipId = editor.session.selectedClipIds[0];
  const hit = sequence && clipId ? findClip(sequence, clipId) : null;
  if (!sequence || !hit) return <p className="empty">クリップを選択</p>;
  const transition = transitionNearClip(hit.track, hit.clip);
  return (
    <EffectControls
      clip={hit.clip}
      playheadTicks={editor.session.playheadTicks}
      transition={transition}
      onEffectsChange={(effects) => {
        editor.execute(projectCommand("エフェクト", (project) => replaceClipEffects(project, sequence.id, hit.clip.id, effects)));
      }}
      onTransitionChange={(next) => {
        editor.execute(projectCommand("トランジション", (project) => replaceTransition(project, sequence.id, next)));
      }}
    />
  );
}

function Color({ editor, sequence }: { editor: Editor; sequence: Sequence | null }) {
  const clipId = editor.session.selectedClipIds[0];
  const hit = sequence && clipId ? findClip(sequence, clipId) : null;
  if (!sequence || !hit) return <p className="empty">クリップを選択</p>;
  const write = (effects: EffectInstance[]) => {
    editor.execute(projectCommand("カラー", (project) => replaceClipEffects(project, sequence.id, hit.clip.id, effects)));
  };
  return (
    <ColorPanel
      effects={hit.clip.effects ?? []}
      onChange={(effect) => {
        const effects = hit.clip.effects ?? [];
        const next = effects.some((item) => item.id === effect.id)
          ? effects.map((item) => (item.id === effect.id ? effect : item))
          : [...effects, effect];
        write(next);
      }}
      onAdd={(effect) => write([...(hit.clip.effects ?? []), effect])}
    />
  );
}

function Audio({ editor, sequence }: { editor: Editor; sequence: Sequence | null }) {
  const [mixer, setMixer] = useState<MixerState>({ masterGainDb: 0, strips: [] });
  useEffect(() => {
    if (!sequence) return;
    setMixer((current) => {
      const fresh = mixerStateFromSequence(sequence);
      return {
        masterGainDb: current.masterGainDb,
        strips: fresh.strips.map((strip) => current.strips.find((item) => item.trackId === strip.trackId) ?? strip),
      };
    });
  }, [sequence]);
  const strips = useMemo(() => (sequence ? stripViews(sequence, mixer, {}) : []), [sequence, mixer]);
  if (!sequence) return <p className="empty">シーケンスがありません</p>;
  return (
    <AudioMixer
      strips={strips}
      masterGainDb={mixer.masterGainDb}
      onMuteChange={(trackId, muted) => editor.execute(commands.patchTrack(sequence.id, trackId, { muted }))}
      onSoloChange={(trackId, solo) => {
        setMixer((current) => ({
          ...current,
          strips: current.strips.map((strip) => (strip.trackId === trackId ? { ...strip, solo } : strip)),
        }));
      }}
      onFaderChange={(trackId, faderDb) => {
        setMixer((current) => ({
          ...current,
          strips: current.strips.map((strip) => (strip.trackId === trackId ? { ...strip, faderDb } : strip)),
        }));
      }}
      onPanChange={(trackId, pan) => {
        setMixer((current) => ({
          ...current,
          strips: current.strips.map((strip) => (strip.trackId === trackId ? { ...strip, pan } : strip)),
        }));
      }}
      onMasterGainChange={(masterGainDb) => setMixer((current) => ({ ...current, masterGainDb }))}
    />
  );
}
