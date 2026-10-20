import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { MediaRef, Project } from "@videocut/model";
import { isProject } from "../model/project-factory";

const MEDIA_EXTENSIONS = ["mp4", "mov", "mxf", "wav", "aac", "mp3", "png", "jpg", "jpeg"];

export interface ImportedFile {
  name: string;
  path: string;
  durationTicks: number;
}

export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function setWindowTitle(title: string): Promise<void> {
  if (!inTauri()) {
    document.title = title;
    return;
  }
  document.title = title;
  await getCurrentWindow().setTitle(title);
}

export async function closeWindow(): Promise<void> {
  if (!inTauri()) return;
  await getCurrentWindow().close();
}

export async function loadProjectFromDisk(): Promise<{ path: string; project: Project } | null> {
  if (inTauri()) {
    const picked = await open({
      multiple: false,
      filters: [{ name: "VideoCut", extensions: ["vcut"] }],
    });
    if (typeof picked !== "string") return null;
    const json = await invokeCommand<string>("read_project", { path: picked });
    return { path: picked, project: parseProject(json) };
  }
  const file = await chooseFiles(".vcut,application/json", false);
  const first = file?.[0];
  if (!first) return null;
  return { path: first.name, project: parseProject(await first.text()) };
}

export async function storeProject(path: string | null, project: Project, saveAs: boolean): Promise<string | null> {
  const json = `${JSON.stringify(project, null, 2)}\n`;
  if (inTauri()) {
    let target = !saveAs && path ? path : null;
    if (!target) {
      const picked = await save({
        defaultPath: `${project.name}.vcut`,
        filters: [{ name: "VideoCut", extensions: ["vcut"] }],
      });
      if (typeof picked !== "string") return null;
      target = picked.toLowerCase().endsWith(".vcut") ? picked : `${picked}.vcut`;
    }
    await invokeCommand("write_project", { path: target, projectJson: json });
    return target;
  }
  download(`${project.name}.vcut`, json);
  return path ?? `${project.name}.vcut`;
}

export async function chooseMediaFiles(): Promise<ImportedFile[] | null> {
  if (inTauri()) {
    const picked = await open({
      multiple: true,
      filters: [{ name: "メディア", extensions: MEDIA_EXTENSIONS }],
    });
    if (!picked) return null;
    const paths = Array.isArray(picked) ? picked : [picked];
    return paths.filter((item): item is string => typeof item === "string").map((filePath) => ({
      name: fileName(filePath),
      path: filePath,
      durationTicks: 0,
    }));
  }
  const files = await chooseFiles(MEDIA_EXTENSIONS.map((extension) => `.${extension}`).join(","), true);
  if (!files?.length) return null;
  return files.map((file) => ({ name: file.name, path: file.name, durationTicks: 0 }));
}

export async function requestFrame(path: string, timeTicks: number, width: number, height: number): Promise<string | null> {
  if (!inTauri() || !path) return null;
  const response = await invokeCommand<{ available: boolean; pngBase64?: string | null }>("frame_at", {
    path,
    timeTicks,
    width,
    height,
  });
  if (!response.available || !response.pngBase64) return null;
  return response.pngBase64;
}

export async function exportStatus(): Promise<string> {
  if (!inTauri()) return "書き出しはデスクトップアプリから実行します。メディア機能が接続されるとエンコードできます";
  return invokeCommand<string>("export_status", {});
}

export type ExchangeFormat = "otio" | "fcpxml" | "edl";

export interface LoadedDocument {
  project: Project;
  warnings: string[];
}

export interface MediaExportRequest {
  sequenceId: string;
  preset: string;
  range: { mode: string; inTicks: number; outTicks: number };
  outputPath: string;
}

export async function loadPrproj(): Promise<LoadedDocument | null> {
  return loadDocument("read_prproj", ["prproj"], ".prproj");
}

export async function storePrproj(project: Project): Promise<string[] | null> {
  return storeDocument(project, "write_prproj", "prproj", null);
}

export async function loadExchange(format: ExchangeFormat): Promise<LoadedDocument | null> {
  return loadDocument("import_interchange", [exchangeExtension(format)], format, { format });
}

export async function storeExchange(
  project: Project,
  format: ExchangeFormat,
  sequenceId: string | null,
): Promise<string[] | null> {
  return storeDocument(project, "export_interchange", exchangeExtension(format), sequenceId, { format });
}

export async function probeMediaFile(filePath: string): Promise<MediaRef> {
  if (!inTauri()) {
    return {
      id: `media-${crypto.randomUUID()}`,
      name: fileName(filePath),
      path: filePath,
      durationTicks: 0,
    };
  }
  const json = await invokeCommand<string>("probe_media", { path: filePath });
  const value: unknown = JSON.parse(json);
  if (!isMediaRef(value)) throw new Error("メディア情報を読めません");
  return value;
}

export async function exportMedia(project: Project, request: MediaExportRequest): Promise<string[]> {
  if (!inTauri()) throw new Error("書き出しはデスクトップアプリから実行します");
  const outcome = await invokeCommand<{ warnings: string[] }>("export_media", {
    projectJson: JSON.stringify(project),
    requestJson: JSON.stringify(request),
  });
  return outcome.warnings ?? [];
}

export async function browseOutputPath(suggested: string): Promise<string | null> {
  if (!inTauri()) return null;
  const picked = await save({
    defaultPath: suggested,
    filters: [
      { name: "H.264", extensions: ["mp4"] },
      { name: "ProRes", extensions: ["mov"] },
      { name: "WAV", extensions: ["wav"] },
    ],
  });
  return typeof picked === "string" ? picked : null;
}

async function invokeCommand<T>(command: string, args: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(messageFrom(error));
  }
}

async function loadDocument(
  command: string,
  extensions: string[],
  label: string,
  extra: Record<string, unknown> = {},
): Promise<LoadedDocument | null> {
  if (!inTauri()) throw new Error("この読み込みはデスクトップアプリから実行します");
  const picked = await open({
    multiple: false,
    filters: [{ name: label, extensions }],
  });
  if (typeof picked !== "string") return null;
  const payload = await invokeCommand<{ projectJson: string; warnings?: string[] }>(command, { path: picked, ...extra });
  return { project: parseProject(payload.projectJson), warnings: payload.warnings ?? [] };
}

async function storeDocument(
  project: Project,
  command: string,
  extension: string,
  sequenceId: string | null,
  extra: Record<string, unknown> = {},
): Promise<string[] | null> {
  if (!inTauri()) throw new Error("この書き出しはデスクトップアプリから実行します");
  const picked = await save({
    defaultPath: `${project.name}.${extension}`,
    filters: [{ name: extension, extensions: [extension] }],
  });
  if (typeof picked !== "string") return null;
  const suffix = `.${extension}`;
  const target = picked.toLowerCase().endsWith(suffix) ? picked : `${picked}${suffix}`;
  const args: Record<string, unknown> = {
    path: target,
    projectJson: JSON.stringify(project),
    ...extra,
  };
  if ("format" in extra) args.sequenceId = sequenceId;
  return invokeCommand<string[]>(command, args);
}

function exchangeExtension(format: ExchangeFormat): string {
  switch (format) {
    case "otio":
      return "otio";
    case "fcpxml":
      return "fcpxml";
    case "edl":
      return "edl";
    default: {
      const unknown: never = format;
      return unknown;
    }
  }
}

function isMediaRef(value: unknown): value is MediaRef {
  if (!value || typeof value !== "object") return false;
  const record = value as Partial<MediaRef>;
  return (
    typeof record.id === "string" &&
    typeof record.name === "string" &&
    typeof record.path === "string" &&
    typeof record.durationTicks === "number"
  );
}

function messageFrom(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error) return error;
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string" && error.message) {
    return error.message;
  }
  return "操作に失敗しました";
}

function parseProject(text: string): Project {
  const value: unknown = JSON.parse(text);
  if (!isProject(value)) throw new Error("VideoCut のプロジェクトとして読めません");
  return value;
}

function fileName(filePath: string): string {
  const parts = filePath.split(/[/\\]/);
  return parts[parts.length - 1] || filePath;
}

function chooseFiles(accept: string, multiple: boolean): Promise<File[] | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = accept;
    input.multiple = multiple;
    input.onchange = () => resolve(input.files ? [...input.files] : null);
    input.oncancel = () => resolve(null);
    input.click();
  });
}

function download(filename: string, text: string): void {
  const blob = new Blob([text], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}
