import type { ReactElement } from "react";
import type { PanelHostProps } from "./panel-props";

export type PanelComponent = (props: PanelHostProps) => ReactElement | null;

export const TIMELINE_EXPORTS = ["Timeline", "TimelineView", "TimelinePanel", "default"] as const;
export const EFFECTS_EXPORTS = ["EffectControls", "EffectsPanel", "default"] as const;
export const COLOR_EXPORTS = ["ColorPanel", "ColorWheels", "default"] as const;
export const AUDIO_EXPORTS = ["AudioMixer", "Mixer", "AudioPanel", "default"] as const;
export const GRAPHICS_EXPORTS = ["GraphicsPanel", "TitlesPanel", "default"] as const;
export const PROJECT_EXPORTS = ["ProjectPanel", "ProjectBin", "default"] as const;
export const EXPORT_EXPORTS = ["ExportPanel", "ExportDialog", "default"] as const;

export function resolvePanel(mod: object | null | undefined, names: readonly string[]): PanelComponent | null {
  if (!mod) return null;
  const record = mod as Record<string, unknown>;
  for (const name of names) {
    const value = record[name];
    if (typeof value === "function") return value as PanelComponent;
  }
  return null;
}
