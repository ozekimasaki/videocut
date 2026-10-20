import { describe, expect, it } from "vitest";
import * as audio from "@videocut/ui-audio";
import * as color from "@videocut/ui-color";
import * as effects from "@videocut/ui-effects";
import * as graphics from "@videocut/ui-graphics";
import * as project from "@videocut/ui-project";
import * as timeline from "@videocut/ui-timeline";
import * as exporter from "@videocut/ui-export";
import { AudioMixer, ColorPanel, EffectControls, ExportPanel, GraphicsPanel, ProjectPanel, Timeline } from "../ui/modules";
import { AUDIO_EXPORTS, COLOR_EXPORTS, EFFECTS_EXPORTS, EXPORT_EXPORTS, GRAPHICS_EXPORTS, PROJECT_EXPORTS, TIMELINE_EXPORTS, resolvePanel } from "./panels";

describe("resolvePanel", () => {
  it("returns null for the local stub", () => {
    expect(resolvePanel({ default: null, available: false }, ["default"])).toBeNull();
  });

  it("returns the first function export", () => {
    const Panel = () => null;
    expect(resolvePanel({ default: null, Timeline: Panel }, ["default", "Timeline"])).toBe(Panel);
  });

  it("resolves the workspace panels", () => {
    const modules: Array<[object, readonly string[]]> = [
      [timeline, TIMELINE_EXPORTS],
      [effects, EFFECTS_EXPORTS],
      [color, COLOR_EXPORTS],
      [audio, AUDIO_EXPORTS],
      [graphics, GRAPHICS_EXPORTS],
      [project, PROJECT_EXPORTS],
      [exporter, EXPORT_EXPORTS],
    ];
    for (const [mod, names] of modules) {
      expect(typeof resolvePanel(mod, names)).toBe("function");
    }
  });

  it("re-exports the panels the shell renders", () => {
    expect(typeof Timeline).toBe("function");
    expect(typeof EffectControls).toBe("function");
    expect(typeof ColorPanel).toBe("function");
    expect(typeof AudioMixer).toBe("function");
    expect(typeof GraphicsPanel).toBe("function");
    expect(typeof ProjectPanel).toBe("function");
    expect(typeof ExportPanel).toBe("function");
  });
});
