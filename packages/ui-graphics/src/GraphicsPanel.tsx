import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import type { Clip, Sequence } from "@videocut/model";
import { commitCaptionTiming } from "./captions.ts";
import { graphicsPanelModel, type CaptionRowModel, type GraphicFields } from "./panelModel.ts";

export type GraphicsPanelProps = {
  sequence: Sequence | null;
  selectedClip: Clip | null;
  onTitleTextChange: (clipId: string, text: string) => void;
  onLowerThirdChange: (clipId: string, primary: string, secondary: string) => void;
  onCaptionTimingChange: (captionId: string, startTicks: number, endTicks: number) => void;
  onCaptionTextChange: (captionId: string, text: string) => void;
};

export function GraphicsPanel(props: GraphicsPanelProps) {
  const model = graphicsPanelModel(props.sequence, props.selectedClip);
  if (model.emptySequence) {
    return <section style={shell}>シーケンスが開かれていません。</section>;
  }
  return (
    <section style={shell}>
      <h2 style={heading}>グラフィックス</h2>
      <GraphicFieldsView
        graphic={model.graphic}
        onTitleTextChange={props.onTitleTextChange}
        onLowerThirdChange={props.onLowerThirdChange}
      />
      <h3 style={subheading}>字幕</h3>
      {model.captions.length === 0 ? (
        <p style={muted}>字幕はありません。</p>
      ) : (
        model.captions.map((caption) => (
          <CaptionRow
            key={caption.id}
            caption={caption}
            onTextChange={props.onCaptionTextChange}
            onTimingChange={props.onCaptionTimingChange}
          />
        ))
      )}
    </section>
  );
}

function GraphicFieldsView(props: {
  graphic: GraphicFields;
  onTitleTextChange: GraphicsPanelProps["onTitleTextChange"];
  onLowerThirdChange: GraphicsPanelProps["onLowerThirdChange"];
}) {
  const graphic = props.graphic;
  switch (graphic.role) {
    case "title":
      return (
        <label style={field}>
          タイトル
          <textarea
            aria-label="タイトル"
            style={input}
            value={graphic.text}
            onChange={(event) => props.onTitleTextChange(graphic.clipId, event.target.value)}
          />
        </label>
      );
    case "lowerThird":
      return (
        <div style={stack}>
          <label style={field}>
            メイン
            <input
              aria-label="ローワーサードのメイン"
              style={input}
              value={graphic.primary}
              onChange={(event) =>
                props.onLowerThirdChange(graphic.clipId, event.target.value, graphic.secondary)
              }
            />
          </label>
          <label style={field}>
            サブ
            <input
              aria-label="ローワーサードのサブ"
              style={input}
              value={graphic.secondary}
              onChange={(event) =>
                props.onLowerThirdChange(graphic.clipId, graphic.primary, event.target.value)
              }
            />
          </label>
        </div>
      );
    case "none":
      return <p style={muted}>クリップを選択するとタイトルを編集できます。</p>;
    default: {
      const unreachable: never = graphic;
      return unreachable;
    }
  }
}

function CaptionRow(props: {
  caption: CaptionRowModel;
  onTextChange: GraphicsPanelProps["onCaptionTextChange"];
  onTimingChange: GraphicsPanelProps["onCaptionTimingChange"];
}) {
  const { caption } = props;
  const [start, setStart] = useState(caption.startLabel);
  const [end, setEnd] = useState(caption.endLabel);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setStart(caption.startLabel);
    setEnd(caption.endLabel);
    setError(null);
  }, [caption.id, caption.startLabel, caption.endLabel]);

  function commit() {
    const result = commitCaptionTiming(start, end);
    if (!result.ok) {
      setError(result.error);
      return;
    }
    setError(null);
    if (
      result.value.startTicks !== caption.startTicks ||
      result.value.endTicks !== caption.endTicks
    ) {
      props.onTimingChange(caption.id, result.value.startTicks, result.value.endTicks);
    }
  }

  return (
    <div style={captionRow}>
      <label style={field}>
        テキスト
        <input
          aria-label={`字幕 ${caption.id} のテキスト`}
          style={input}
          value={caption.text}
          onChange={(event) => props.onTextChange(caption.id, event.target.value)}
        />
      </label>
      <label style={field}>
        開始
        <input
          aria-label={`字幕 ${caption.id} の開始`}
          style={input}
          spellCheck={false}
          value={start}
          onChange={(event) => setStart(event.target.value)}
          onBlur={commit}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.currentTarget.blur();
            }
          }}
        />
      </label>
      <label style={field}>
        終了
        <input
          aria-label={`字幕 ${caption.id} の終了`}
          style={input}
          spellCheck={false}
          value={end}
          onChange={(event) => setEnd(event.target.value)}
          onBlur={commit}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.currentTarget.blur();
            }
          }}
        />
      </label>
      {error ? (
        <p role="alert" style={errorText}>
          {error}
        </p>
      ) : null}
    </div>
  );
}

const shell: CSSProperties = {
  boxSizing: "border-box",
  height: "100%",
  overflow: "auto",
  padding: 12,
  background: "#1c1c1c",
  color: "#e8e8e8",
  fontFamily: "system-ui, sans-serif",
  fontSize: 13,
};

const heading: CSSProperties = {
  margin: "0 0 12px",
  fontSize: 14,
  fontWeight: 600,
};

const subheading: CSSProperties = {
  margin: "16px 0 8px",
  fontSize: 12,
  fontWeight: 600,
  color: "#c8c8c8",
};

const muted: CSSProperties = {
  margin: 0,
  color: "#9a9a9a",
};

const stack: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 8,
};

const field: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 4,
  color: "#c8c8c8",
};

const input: CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  border: "1px solid #3a3a3a",
  borderRadius: 4,
  background: "#242424",
  color: "#e8e8e8",
  padding: "6px 8px",
  font: "inherit",
};

const captionRow: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  marginBottom: 12,
  paddingBottom: 12,
  borderBottom: "1px solid #3a3a3a",
};

const errorText: CSSProperties = {
  margin: 0,
  color: "#e0b4b4",
};
