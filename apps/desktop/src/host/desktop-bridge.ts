import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { Project } from "@videocut/model";
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
    const json = await invoke<string>("read_project", { path: picked });
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
    await invoke("write_project", { path: target, project_json: json });
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
  const response = await invoke<{ available: boolean; pngBase64?: string | null }>("frame_at", {
    path,
    time_ticks: timeTicks,
    width,
    height,
  });
  if (!response.available || !response.pngBase64) return null;
  return response.pngBase64;
}

export async function exportStatus(): Promise<string> {
  if (!inTauri()) return "書き出しはデスクトップアプリから実行します。メディア機能が接続されるとエンコードできます";
  return invoke<string>("export_status");
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
