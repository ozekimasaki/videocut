import assert from "node:assert/strict";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { Sequence } from "@videocut/model";
import { TICKS_PER_SECOND } from "@videocut/model";
import { Timeline } from "./Timeline.tsx";

const sequence: Sequence = {
  id: "seq",
  name: "seq",
  settings: {
    width: 1920,
    height: 1080,
    fps: { num: 30, den: 1 },
    sampleRate: 48_000,
    audioChannels: 2,
    startTimecodeTicks: 0,
  },
  videoTracks: [
    {
      id: "v1",
      index: 0,
      kind: "video",
      clips: [
        {
          id: "clip-a",
          name: "interview",
          mediaId: "media-1",
          kind: "media",
          timelineIn: 0,
          timelineOut: TICKS_PER_SECOND,
          sourceIn: 0,
          sourceOut: TICKS_PER_SECOND,
          speed: 1,
          reversed: false,
          enabled: true,
          labelColor: 0,
        },
      ],
      locked: false,
      muted: false,
      hidden: false,
      syncLock: true,
      targeted: true,
    },
  ],
  audioTracks: [],
  markers: [
    {
      id: "marker-1",
      timeTicks: 0,
      durationTicks: 0,
      name: "head",
      comment: "",
      color: 1,
      kind: "comment",
    },
  ],
};

test("timeline renders tools, tracks, and clips", () => {
  const html = renderToStaticMarkup(createElement(Timeline, { sequence, tool: "razor" }));
  assert.match(html, /data-testid="timeline"/);
  assert.match(html, /interview/);
  assert.match(html, /レーザー/);
  assert.match(html, /aria-pressed="true"/);
  assert.match(html, /V1/);
  assert.match(html, /ターゲット/);
  assert.match(html, /シンクロック/);
  assert.match(html, /リフト/);
  assert.match(html, /抽出/);
  assert.match(html, /ネスト/);
  assert.match(html, /静止画/);
  assert.match(html, /アングル切替/);
});
