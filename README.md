# VideoCut

Windows 向けのオープンソース・ノンリニアエディタ。

Adobe とは無関係です。Premiere の名称、ロゴ、アイコン、付属エフェクトのバイナリは含みません。画面は業務用 NLE の標準的なパネル構成（プロジェクト、ソース、プログラム、タイムライン、エフェクト）です。

## 互換

- メディアの読み込みと書き出し（FFmpeg）
- FCPXML、CMX 3600 EDL、OpenTimelineIO
- `.prproj`（gzip された XML）の読み書き

## 構成

- デスクトップ: Tauri 2 + React + TypeScript（Windows）
- 編集データ: Rust クレート `videocut-model`
- 時間単位: 1 秒 = 254016000000 tick
- ネイティブのプロジェクト拡張子: `.vcut`（JSON）

Rust の `crates/model` がデータ型の正です。TypeScript の `packages/model` は同じフィールド名に合わせます。
