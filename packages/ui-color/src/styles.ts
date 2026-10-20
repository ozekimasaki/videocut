export const COLOR_PANEL_CSS = `
.vc-color {
  --vc-bg: #1a1d1f;
  --vc-panel: #24282c;
  --vc-line: #3a4148;
  --vc-text: #e7ecec;
  --vc-muted: #9aa3a8;
  --vc-accent: #3d9a8f;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 100%;
  padding: 12px;
  color: var(--vc-text);
  background: var(--vc-bg);
  font: 12px/1.4 "Segoe UI", sans-serif;
}
.vc-color * { box-sizing: border-box; }
.vc-color__header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}
.vc-color__header h2 {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.vc-color__tabs, .vc-color__chips, .vc-color__channels, .vc-color__scope-modes {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.vc-color button, .vc-color input, .vc-color textarea {
  color: inherit;
  font: inherit;
}
.vc-color__tab, .vc-color__chip, .vc-color__channels button, .vc-color__scope-modes button, .vc-color__add button, .vc-color__reset {
  border: 1px solid var(--vc-line);
  border-radius: 4px;
  background: var(--vc-panel);
  padding: 4px 8px;
  cursor: pointer;
}
.vc-color__tab.is-active, .vc-color__chip.is-active, .vc-color__channels button.is-active, .vc-color__scope-modes button.is-active {
  border-color: var(--vc-accent);
  color: white;
  background: #2c6f67;
}
.vc-color__editor, .vc-color__scopes {
  border: 1px solid var(--vc-line);
  border-radius: 6px;
  background: var(--vc-panel);
  padding: 10px;
}
.vc-color__wheels {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}
.vc-color__wheel { display: flex; flex-direction: column; align-items: center; gap: 6px; }
.vc-color__wheel svg {
  width: 100%;
  max-width: 160px;
  height: auto;
  background: radial-gradient(circle, #2a2f33 0%, #171a1c 70%);
  border-radius: 50%;
  touch-action: none;
  cursor: crosshair;
}
.vc-color__slider { display: flex; flex-direction: column; gap: 2px; width: 100%; }
.vc-color__slider label, .vc-color__muted { color: var(--vc-muted); }
.vc-color__slider input[type="range"], .vc-color__wheel input[type="range"] { width: 100%; accent-color: var(--vc-accent); }
.vc-color__curve { width: 100%; height: 220px; background: #15181a; border-radius: 4px; touch-action: none; }
.vc-color__hsl, .vc-color__lut { display: flex; flex-direction: column; gap: 8px; }
.vc-color textarea {
  min-height: 96px;
  width: 100%;
  resize: vertical;
  border: 1px solid var(--vc-line);
  border-radius: 4px;
  background: #15181a;
  padding: 6px;
}
.vc-color__empty { margin: 0; color: var(--vc-muted); }
.vc-color__scope-canvas {
  display: block;
  width: 100%;
  height: 160px;
  margin-top: 8px;
  background: #101214;
  image-rendering: pixelated;
}
.vc-color__add { display: flex; flex-wrap: wrap; gap: 4px; }
`;
