import assert from "node:assert/strict";
import { test } from "node:test";
import { TICKS_PER_SECOND } from "@videocut/model";
import type { Clip, Sequence, Track } from "@videocut/model";
import {
  EXPORT_PRESETS,
  ExportValidationError,
  buildExportRequest,
  formatTimecode,
  isExportPresetId,
  presetExtension,
  resolveExportRange,
  sequenceEndTicks,
  submitExport,
  suggestOutputPath,
  validateExportForm,
  withPresetExtension,
} from "./export";
import type { ExportRequest, MediaExporter } from "./export";

const fps30 = { num: 30, den: 1 };
const frame30 = TICKS_PER_SECOND / 30;

function clip(id: string, timelineOut: number): Clip {
  return {
    id,
    name: id,
    kind: "media",
    timelineIn: 0,
    timelineOut,
    sourceIn: 0,
    sourceOut: timelineOut,
    speed: 1,
    reversed: false,
    enabled: true,
    labelColor: 0,
  };
}

function track(id: string, kind: Track["kind"], clips: Clip[]): Track {
  return {
    id,
    index: 0,
    kind,
    clips,
    locked: false,
    muted: false,
    hidden: false,
    syncLock: true,
    targeted: true,
  };
}

function sequence(videoOut = 0, audioOut = 0): Sequence {
  return {
    id: "seq-1",
    name: "Interview",
    settings: {
      width: 1920,
      height: 1080,
      fps: fps30,
      sampleRate: 48_000,
      audioChannels: 2,
      startTimecodeTicks: 0,
    },
    videoTracks: videoOut > 0 ? [track("v1", "video", [clip("v", videoOut)])] : [],
    audioTracks: audioOut > 0 ? [track("a1", "audio", [clip("a", audioOut)])] : [],
  };
}

test("presets are H.264, ProRes, and WAV", () => {
  assert.deepEqual(
    EXPORT_PRESETS.map((preset) => [preset.id, preset.label, preset.extension]),
    [
      ["h264", "H.264", ".mp4"],
      ["prores", "ProRes", ".mov"],
      ["wav", "WAV", ".wav"],
    ],
  );
  assert.equal(presetExtension("h264"), ".mp4");
  assert.equal(presetExtension("prores"), ".mov");
  assert.equal(presetExtension("wav"), ".wav");
  assert.equal(isExportPresetId("h264"), true);
  assert.equal(isExportPresetId("h265"), false);
});

test("sequence end is the later of the video and audio edges", () => {
  assert.equal(sequenceEndTicks(sequence()), 0);
  assert.equal(sequenceEndTicks(sequence(100, 250)), 250);
});

test("entire range covers the sequence and in/out keeps the marks", () => {
  const entire = resolveExportRange(sequence(frame30 * 90), { mode: "entire" });
  assert.deepEqual(entire, { mode: "entire", inTicks: 0, outTicks: frame30 * 90 });
  const marked = resolveExportRange(sequence(frame30 * 90), {
    mode: "inOut",
    inTicks: frame30,
    outTicks: frame30 * 10,
  });
  assert.deepEqual(marked, { mode: "inOut", inTicks: frame30, outTicks: frame30 * 10 });
});

test("range validation rejects empty, missing, inverted, and fractional marks", () => {
  assert.throws(() => resolveExportRange(sequence(), { mode: "entire" }), /no duration/);
  assert.throws(
    () => resolveExportRange(sequence(10), { mode: "inOut", outTicks: 10 }),
    /required/,
  );
  assert.throws(
    () => resolveExportRange(sequence(10), { mode: "inOut", inTicks: 4, outTicks: 4 }),
    /after the in point/,
  );
  assert.throws(
    () => resolveExportRange(sequence(10), { mode: "inOut", inTicks: -1, outTicks: 4 }),
    /negative/,
  );
  assert.throws(
    () => resolveExportRange(sequence(10), { mode: "inOut", inTicks: 1.5, outTicks: 4 }),
    /whole ticks/,
  );
});

test("output paths take the preset extension", () => {
  assert.equal(withPresetExtension("  interview.MP4 ", "prores"), "interview.mov");
  assert.equal(withPresetExtension("interview", "wav"), "interview.wav");
  assert.equal(withPresetExtension("interview.mp4.bak", "wav"), "interview.mp4.bak.wav");
  assert.equal(withPresetExtension("   ", "h264"), "");
  assert.equal(suggestOutputPath("My:Seq", "h264"), "My_Seq.mp4");
  assert.equal(suggestOutputPath("Interview", "wav", "/out/"), "/out/Interview.wav");
  assert.equal(suggestOutputPath("Interview", "prores", "D:\\out\\"), "D:\\out\\Interview.mov");
  assert.equal(suggestOutputPath("   ", "h264", "/out"), "/out/sequence.mp4");
});

test("buildExportRequest returns the shell contract", () => {
  const request = buildExportRequest({
    sequence: sequence(frame30 * 30),
    preset: "h264",
    range: { mode: "entire" },
    outputPath: "renders/interview",
  });
  assert.deepEqual(request, {
    sequenceId: "seq-1",
    preset: "h264",
    range: { mode: "entire", inTicks: 0, outTicks: frame30 * 30 },
    outputPath: "renders/interview.mp4",
  } satisfies ExportRequest);
  assert.throws(
    () =>
      buildExportRequest({
        sequence: sequence(10),
        preset: "wav",
        range: { mode: "entire" },
        outputPath: "  ",
      }),
    ExportValidationError,
  );
});

test("timecode counts whole frames", () => {
  assert.equal(formatTimecode(0, fps30), "00:00:00:00");
  assert.equal(formatTimecode(frame30, fps30), "00:00:00:01");
  assert.equal(formatTimecode(TICKS_PER_SECOND, fps30), "00:00:01:00");
  const film = { num: 24_000, den: 1001 };
  const filmFrame = Math.round((TICKS_PER_SECOND * film.den) / film.num);
  assert.equal(formatTimecode(filmFrame * 24, film), "00:00:01:00");
  assert.throws(() => formatTimecode(0, { num: 0, den: 1 }), ExportValidationError);
});

test("the form and exporter seam", async () => {
  const seq = sequence(frame30 * 12);
  const missing = validateExportForm([], {
    sequenceId: "seq-1",
    preset: "h264",
    rangeMode: "entire",
    outputPath: "out.mp4",
  }, { inTicks: null, outTicks: null });
  assert.equal(missing.ok, false);

  const invalid = validateExportForm([seq], {
    sequenceId: "seq-1",
    preset: "wav",
    rangeMode: "inOut",
    outputPath: "out.wav",
  }, { inTicks: null, outTicks: 10 });
  assert.equal(invalid.ok, false);

  const valid = validateExportForm([seq], {
    sequenceId: "seq-1",
    preset: "prores",
    rangeMode: "inOut",
    outputPath: "D:\\renders\\cut.mp4",
  }, { inTicks: 0, outTicks: frame30 * 4 });
  assert.equal(valid.ok, true);
  if (!valid.ok) return;
  assert.equal(valid.request.preset, "prores");
  assert.equal(valid.request.outputPath, "D:\\renders\\cut.mov");
  assert.deepEqual(valid.request.range, { mode: "inOut", inTicks: 0, outTicks: frame30 * 4 });

  const seen: ExportRequest[] = [];
  const exporter: MediaExporter = {
    async exportSequence(request) {
      seen.push(request);
    },
  };
  assert.deepEqual(await submitExport(undefined, valid), {
    ok: false,
    message: "Media export is not connected.",
  });
  assert.deepEqual(await submitExport(exporter, invalid), invalid);
  assert.deepEqual(await submitExport(exporter, valid), { ok: true });
  assert.equal(seen.length, 1);
  assert.equal(seen[0]?.sequenceId, "seq-1");

  const failing: MediaExporter = {
    async exportSequence() {
      throw new Error("encoder missing");
    },
  };
  assert.deepEqual(await submitExport(failing, valid), { ok: false, message: "encoder missing" });
});
