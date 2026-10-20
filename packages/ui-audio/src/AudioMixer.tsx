import type { ChangeEvent } from "react";

import {
  clampFader,
  clampPan,
  FADER_MAX_DB,
  FADER_MIN_DB,
  formatDb,
  formatPan,
  meterFraction,
  PAN_MAX,
  PAN_MIN,
} from "./levels";
import type { MeterReading, MixerStripView } from "./types";
import "./mixer.css";

export interface AudioMixerProps {
  strips: readonly MixerStripView[];
  masterGainDb: number;
  masterMeters?: readonly MeterReading[];
  onMuteChange?: (trackId: string, muted: boolean) => void;
  onSoloChange?: (trackId: string, solo: boolean) => void;
  onFaderChange?: (trackId: string, faderDb: number) => void;
  onPanChange?: (trackId: string, pan: number) => void;
  onMasterGainChange?: (gainDb: number) => void;
}

export function AudioMixer(props: AudioMixerProps) {
  const masterMeters = props.masterMeters ?? [];
  return (
    <section className="vcut-audio" aria-label="オーディオミキサー">
      <div className="vcut-audio__strips">
        {props.strips.map((strip) => (
          <Strip
            key={strip.trackId}
            strip={strip}
            onMuteChange={props.onMuteChange}
            onSoloChange={props.onSoloChange}
            onFaderChange={props.onFaderChange}
            onPanChange={props.onPanChange}
          />
        ))}
        <article className="vcut-audio__strip vcut-audio__strip--master" aria-label="マスター">
          <h2 className="vcut-audio__name">マスター</h2>
          <MeterBank readings={masterMeters} label="マスター" />
          <Fader
            label="マスターフェーダー"
            value={props.masterGainDb}
            onChange={(value) => props.onMasterGainChange?.(value)}
          />
        </article>
      </div>
    </section>
  );
}

function Strip(props: {
  strip: MixerStripView;
  onMuteChange?: (trackId: string, muted: boolean) => void;
  onSoloChange?: (trackId: string, solo: boolean) => void;
  onFaderChange?: (trackId: string, faderDb: number) => void;
  onPanChange?: (trackId: string, pan: number) => void;
}) {
  const { strip } = props;
  const silent = !strip.audible;
  return (
    <article
      className={silent ? "vcut-audio__strip is-silent" : "vcut-audio__strip"}
      aria-label={strip.name}
      data-track-id={strip.trackId}
    >
      <h2 className="vcut-audio__name">{strip.name}</h2>
      <div className="vcut-audio__toggles">
        <button
          type="button"
          className={strip.muted ? "vcut-audio__toggle is-mute" : "vcut-audio__toggle"}
          aria-pressed={strip.muted}
          aria-label={`${strip.name} ミュート`}
          onClick={() => props.onMuteChange?.(strip.trackId, !strip.muted)}
        >
          M
        </button>
        <button
          type="button"
          className={strip.solo ? "vcut-audio__toggle is-solo" : "vcut-audio__toggle"}
          aria-pressed={strip.solo}
          aria-label={`${strip.name} ソロ`}
          onClick={() => props.onSoloChange?.(strip.trackId, !strip.solo)}
        >
          S
        </button>
      </div>
      <MeterBank readings={strip.meters} label={strip.name} />
      <Fader
        label={`${strip.name} フェーダー`}
        value={strip.faderDb}
        onChange={(value) => props.onFaderChange?.(strip.trackId, value)}
      />
      <label className="vcut-audio__pan">
        <span className="vcut-audio-sr">{`${strip.name} パン`}</span>
        <input
          type="range"
          min={PAN_MIN}
          max={PAN_MAX}
          step={0.01}
          value={strip.pan}
          aria-label={`${strip.name} パン`}
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            props.onPanChange?.(strip.trackId, clampPan(Number(event.target.value)))
          }
        />
        <span className="vcut-audio__readout">{formatPan(strip.pan)}</span>
      </label>
    </article>
  );
}

function Fader(props: { label: string; value: number; onChange: (value: number) => void }) {
  return (
    <label className="vcut-audio__fader">
      <span className="vcut-audio-sr">{props.label}</span>
      <input
        className="vcut-audio__fader-input"
        type="range"
        min={FADER_MIN_DB}
        max={FADER_MAX_DB}
        step={0.1}
        value={props.value}
        aria-label={props.label}
        onChange={(event: ChangeEvent<HTMLInputElement>) =>
          props.onChange(clampFader(Number(event.target.value)))
        }
      />
      <span className="vcut-audio__readout">{formatDb(props.value)}</span>
    </label>
  );
}

function MeterBank(props: { readings: readonly MeterReading[]; label: string }) {
  const readings = props.readings.length > 0 ? props.readings : [{ peakDbfs: -120 }];
  return (
    <div className="vcut-audio__meters" aria-hidden="true">
      {readings.map((reading, index) => {
        const clipped = reading.clipped === true || reading.peakDbfs >= 0;
        return (
          <div className="vcut-audio-meter" key={`${props.label}-${index}`}>
            <div
              className="vcut-audio-meter__fill"
              style={{ height: `${meterFraction(reading.peakDbfs) * 100}%` }}
            />
            {clipped ? <div className="vcut-audio-meter__clip" /> : null}
          </div>
        );
      })}
    </div>
  );
}
