import assert from "node:assert/strict";
import { test } from "node:test";
import { TICKS_PER_SECOND } from "@videocut/model";
import type { BinNode, Project, Sequence } from "@videocut/model";
import {
  BinEditError,
  addImportedMedia,
  binKindLabel,
  binPath,
  createBin,
  formatMediaDuration,
  mediaFromPath,
  renameBinItem,
  visibleBinNodes,
} from "./bins";

const settings: Sequence["settings"] = {
  width: 1920,
  height: 1080,
  fps: { num: 30, den: 1 },
  sampleRate: 48_000,
  audioChannels: 2,
  startTimecodeTicks: 0,
};

function sample(): Project {
  return {
    id: "proj-1",
    name: "Spot",
    bins: [
      {
        id: "bin-a",
        name: "Interviews",
        kind: "bin",
        children: [
          { id: "clip-1", name: "Take A", kind: "clip", mediaId: "media-1" },
          { id: "clip-2", name: "Take A", kind: "clip", mediaId: "media-1" },
          {
            id: "bin-b",
            name: "Selects",
            kind: "bin",
            children: [
              { id: "seq-node", name: "Rough Cut", kind: "sequence", sequenceId: "seq-1" },
              { id: "seq-node-2", name: "Rough Cut", kind: "sequence", sequenceId: "seq-1" },
            ],
          },
        ],
      },
    ],
    sequences: [
      {
        id: "seq-1",
        name: "Rough Cut",
        settings,
        videoTracks: [],
        audioTracks: [],
      },
    ],
    media: [
      {
        id: "media-1",
        name: "Take A",
        path: "/media/take-a.mp4",
        durationTicks: TICKS_PER_SECOND * 12,
      },
    ],
  };
}

function freeze<T>(value: T): T {
  if (value && typeof value === "object" && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const item of Object.values(value)) freeze(item);
  }
  return value;
}

test("bin kind labels cover every item kind", () => {
  assert.equal(binKindLabel("bin"), "Bin");
  assert.equal(binKindLabel("clip"), "Clip");
  assert.equal(binKindLabel("sequence"), "Sequence");
});

test("createBin appends a named bin without mutating the project", () => {
  const project = freeze(sample());
  const next = createBin(project, null, { name: "Audio", id: "bin-audio" });
  assert.equal(next.bins.at(-1)?.id, "bin-audio");
  assert.equal(next.bins.at(-1)?.kind, "bin");
  assert.equal(next.bins.at(-1)?.name, "Audio");
  assert.equal(project.bins.length, 1);
});

test("createBin nests under a bin and rejects other parents", () => {
  const project = sample();
  const nested = createBin(project, "bin-b", { name: "Picks", id: "bin-picks" });
  const selects = nested.bins[0]?.children?.[2];
  assert.equal(selects?.kind, "bin");
  assert.equal(selects?.children?.at(-1)?.name, "Picks");
  assert.throws(() => createBin(project, "missing", { name: "Nope" }), BinEditError);
  assert.throws(() => createBin(project, "clip-1", { name: "Nope" }), BinEditError);
});

test("createBin picks a unique default name", () => {
  const project = createBin(sample(), null, { name: "Bin", id: "bin-1" });
  const next = createBin(project, null, { id: "bin-2" });
  assert.equal(next.bins.at(-1)?.name, "Bin 2");
  const again = createBin(next, null, { id: "bin-3" });
  assert.equal(again.bins.at(-1)?.name, "Bin 3");
});

test("rename updates a bin, shared clips, and shared sequences", () => {
  const project = freeze(sample());
  const bin = renameBinItem(project, "bin-a", "Talks");
  assert.equal(bin.bins[0]?.name, "Talks");
  assert.equal(bin.media[0]?.name, "Take A");
  assert.equal(project.bins[0]?.name, "Interviews");

  const clip = renameBinItem(project, "clip-1", "Hero");
  assert.equal(clip.media[0]?.name, "Hero");
  const interviews = clip.bins[0]?.children ?? [];
  assert.equal(interviews[0]?.name, "Hero");
  assert.equal(interviews[1]?.name, "Hero");

  const sequence = renameBinItem(project, "seq-node", "Fine Cut");
  assert.equal(sequence.sequences[0]?.name, "Fine Cut");
  const selects = sequence.bins[0]?.children?.[2]?.children ?? [];
  assert.equal(selects[0]?.name, "Fine Cut");
  assert.equal(selects[1]?.name, "Fine Cut");
});

test("rename ignores blank text and an unchanged name", () => {
  const project = sample();
  assert.equal(renameBinItem(project, "bin-a", "   "), project);
  assert.equal(renameBinItem(project, "bin-a", "Interviews"), project);
  assert.throws(() => renameBinItem(project, "missing", "X"), BinEditError);
});

test("import adds media and a clip, and reuses an existing path", () => {
  const project = freeze(sample());
  const imported = addImportedMedia(project, "bin-b", [
    {
      id: "media-2",
      name: "Room tone",
      path: "/media/room.wav",
      durationTicks: TICKS_PER_SECOND,
    },
  ]);
  assert.equal(imported.media.length, 2);
  const selects = imported.bins[0]?.children?.[2];
  const clip = selects?.children?.at(-1);
  assert.equal(clip?.kind, "clip");
  assert.equal(clip?.mediaId, "media-2");
  assert.equal(clip?.name, "Room tone");
  assert.equal(project.media.length, 1);

  const duplicate = addImportedMedia(imported, null, [
    {
      id: "media-other",
      name: "Room tone copy",
      path: "/media/room.wav",
      durationTicks: TICKS_PER_SECOND,
    },
  ]);
  assert.equal(duplicate.media.length, 2);
  assert.equal(duplicate.bins.at(-1)?.mediaId, "media-2");
  assert.equal(duplicate.bins.at(-1)?.name, "Room tone");

  const sameBin = addImportedMedia(duplicate, null, [
    {
      id: "media-2",
      name: "Room tone",
      path: "/media/room.wav",
      durationTicks: TICKS_PER_SECOND,
    },
  ]);
  assert.equal(sameBin, duplicate);
});

test("import of nothing, or into a clip, does not invent items", () => {
  const project = sample();
  assert.equal(addImportedMedia(project, null, []), project);
  assert.throws(
    () =>
      addImportedMedia(project, "clip-1", [
        { id: "media-3", name: "x", path: "/x.mp4", durationTicks: 1 },
      ]),
    BinEditError,
  );
  assert.throws(
    () => addImportedMedia(project, null, [{ id: " ", name: "x", path: "/x.mp4", durationTicks: 1 }]),
    BinEditError,
  );
});

test("search stays inside the open bin and matches nested names", () => {
  const project = sample();
  assert.deepEqual(
    visibleBinNodes(project, null, "").map((node) => node.id),
    ["bin-a"],
  );
  assert.deepEqual(
    visibleBinNodes(project, "bin-a", "take a").map((node) => node.name),
    ["Take A", "Take A"],
  );
  assert.deepEqual(
    visibleBinNodes(project, null, "rough").map((node) => node.id),
    ["seq-node", "seq-node-2"],
  );
  assert.deepEqual(visibleBinNodes(project, "bin-b", "Interviews"), []);
  assert.deepEqual(visibleBinNodes(project, "missing", ""), []);
});

test("bin path lists ancestors from the root", () => {
  const path = binPath(sample().bins, "bin-b").map((node: BinNode) => node.name);
  assert.deepEqual(path, ["Interviews", "Selects"]);
  assert.deepEqual(binPath(sample().bins, null), []);
});

test("duration and file names", () => {
  assert.equal(formatMediaDuration(0), null);
  assert.equal(formatMediaDuration(TICKS_PER_SECOND * 12), "0:12");
  assert.equal(formatMediaDuration(TICKS_PER_SECOND * 3661), "1:01:01");
  const windows = mediaFromPath("C:\\media\\take.mp4", "media-9");
  assert.equal(windows.name, "take.mp4");
  assert.equal(windows.path, "C:\\media\\take.mp4");
  assert.equal(windows.id, "media-9");
  assert.equal(mediaFromPath("/a/b/c.wav").name, "c.wav");
});
