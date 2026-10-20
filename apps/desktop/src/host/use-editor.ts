import { useCallback, useEffect, useRef, useState } from "react";
import { TICKS_PER_SECOND, type Project } from "@videocut/model";
import { createDemoProject, createEmptyProject } from "../model/project-factory";
import { ticksPerFrame } from "../model/timecode";
import type { EditCommand } from "./command-stack";
import { CommandStack } from "./command-stack";
import {
  chooseMediaFiles,
  closeWindow,
  loadExchange,
  loadProjectFromDisk,
  loadPrproj,
  probeMediaFile,
  setWindowTitle,
  storeExchange,
  storeProject,
  storePrproj,
  type ExchangeFormat,
} from "./desktop-bridge";
import { commands } from "./edits";
import { actionFromKey, nextShuttleSpeed, type KeyAction } from "./keyboard";
import { clipsStartingAt, findClip, sequenceById, sequenceEnd } from "./playhead";
import type { SessionState, ToolId } from "./types";

function initialSession(project: Project, path: string | null = null): SessionState {
  return {
    activeSequenceId: project.sequences[0]?.id ?? null,
    tool: "select",
    selectedClipIds: [],
    playheadTicks: 0,
    sourceMediaId: null,
    sourceClipId: null,
    sourcePlayheadTicks: 0,
    sourceMarkIn: null,
    sourceMarkOut: null,
    programMarkIn: null,
    programMarkOut: null,
    focusedMonitor: "program",
    playing: null,
    projectPath: path,
    message: "準備完了",
  };
}

function clamp(value: number, min: number, max: number): number {
  if (max < min) return min;
  return Math.min(max, Math.max(min, value));
}

function movePlayhead(session: SessionState, project: Project, delta: number): SessionState {
  if (!session.playing || delta === 0) return session;
  if (session.playing.monitor === "source") {
    const duration = sourceDuration(project, session);
    const next = clamp(session.sourcePlayheadTicks + delta, 0, Math.max(duration, 0));
    const stopped = (delta > 0 && next >= duration) || (delta < 0 && next <= 0);
    if (next === session.sourcePlayheadTicks && !stopped) return session;
    return {
      ...session,
      sourcePlayheadTicks: next,
      playing: stopped ? null : session.playing,
      message: stopped ? "ソースの終わり" : session.message,
    };
  }
  const sequence = sequenceById(project, session.activeSequenceId);
  const duration = sequence ? Math.max(sequenceEnd(sequence), ticksPerFrame(sequence.settings.fps)) : 0;
  const next = clamp(session.playheadTicks + delta, 0, duration);
  const stopped = duration === 0 || (delta > 0 && next >= duration) || (delta < 0 && next <= 0);
  if (next === session.playheadTicks && !stopped) return session;
  return {
    ...session,
    playheadTicks: next,
    playing: stopped ? null : session.playing,
      message: stopped ? "シーケンスの終わり" : session.message,
  };
}

function sourceDuration(project: Project, session: SessionState): number {
  if (session.sourceMediaId) {
    return project.media.find((item) => item.id === session.sourceMediaId)?.durationTicks ?? 0;
  }
  const sequence = sequenceById(project, session.activeSequenceId);
  if (!sequence || !session.sourceClipId) return 0;
  const hit = findClip(sequence, session.sourceClipId);
  if (!hit) return 0;
  return Math.max(0, hit.clip.sourceOut - hit.clip.sourceIn);
}

function shuttle(session: SessionState, direction: -1 | 1): SessionState {
  const monitor = session.focusedMonitor;
  const same = session.playing?.monitor === monitor && session.playing.direction === direction;
  return {
    ...session,
    playing: {
      monitor,
      direction,
      speed: nextShuttleSpeed(session.playing?.speed ?? 1, Boolean(same)),
    },
  };
}

export function useEditor() {
  const stackRef = useRef<CommandStack | null>(null);
  if (stackRef.current === null) stackRef.current = new CommandStack(createDemoProject());
  const stack = stackRef.current;
  const [project, setProject] = useState(stack.project);
  const [session, setSession] = useState(() => initialSession(stack.project));
  const [preview, setPreview] = useState<Project | null>(null);
  const [revision, setRevision] = useState(0);
  const projectRef = useRef(project);
  const sessionRef = useRef(session);
  const shown = preview ?? project;
  projectRef.current = shown;
  sessionRef.current = session;

  const refresh = useCallback(() => {
    setProject(stack.project);
    setRevision((value) => value + 1);
  }, [stack]);

  const execute = useCallback(
    (command: EditCommand): Project | null => {
      setPreview(null);
      setSession((current) => (current.playing ? { ...current, playing: null } : current));
      if (!stack.execute(command)) return null;
      refresh();
      return stack.project;
    },
    [refresh, stack],
  );

  const undo = useCallback(() => {
    setPreview(null);
    if (!stack.undo()) return;
    refresh();
  }, [refresh, stack]);

  const redo = useCallback(() => {
    setPreview(null);
    if (!stack.redo()) return;
    refresh();
  }, [refresh, stack]);

  const replaceDocument = useCallback(
    (next: Project, path: string | null, message: string) => {
      setPreview(null);
      stack.reset(next);
      setProject(next);
      setSession(initialSession(next, path));
      setSession((current) => ({ ...current, message }));
      setRevision((value) => value + 1);
    },
    [stack],
  );

  const save = useCallback(
    async (saveAs = false) => {
      try {
        const stored = await storeProject(sessionRef.current.projectPath, stack.project, saveAs);
        if (!stored) return;
        stack.markSaved();
        setSession((current) => ({ ...current, projectPath: stored, message: "保存しました" }));
        setRevision((value) => value + 1);
      } catch (error) {
        setSession((current) => ({ ...current, message: errorText(error) }));
      }
    },
    [stack],
  );

  const open = useCallback(async () => {
    try {
      const loaded = await loadProjectFromDisk();
      if (!loaded) return;
      replaceDocument(loaded.project, loaded.path, "プロジェクトを開きました");
    } catch (error) {
      setSession((current) => ({ ...current, message: errorText(error) }));
    }
  }, [replaceDocument]);

  const importMedia = useCallback(async () => {
    try {
      const files = await chooseMediaFiles();
      if (!files?.length) return;
      const probed = [];
      for (const file of files) probed.push(await probeMediaFile(file.path));
      execute(commands.importMedia(probed));
      setSession((current) => ({ ...current, message: `${probed.length} 件読み込みました` }));
    } catch (error) {
      setSession((current) => ({ ...current, message: errorText(error) }));
    }
  }, [execute]);

  const applyAction = useCallback(
    (action: KeyAction) => {
      const current = sessionRef.current;
      const sequenceId = current.activeSequenceId;
      switch (action.type) {
        case "tool":
          setSession((value) => ({ ...value, tool: action.tool }));
          return;
        case "undo":
          undo();
          return;
        case "redo":
          redo();
          return;
        case "save":
          void save(false);
          return;
        case "shuttle-forward":
          setSession((value) => shuttle(value, 1));
          return;
        case "shuttle-reverse":
          setSession((value) => shuttle(value, -1));
          return;
        case "shuttle-stop":
          setSession((value) => ({ ...value, playing: null }));
          return;
        case "toggle-play":
          setSession((value) =>
            value.playing
              ? { ...value, playing: null }
              : { ...value, playing: { monitor: value.focusedMonitor, direction: 1, speed: 1 } },
          );
          return;
        case "mark-in":
          setSession((value) =>
            value.focusedMonitor === "source"
              ? { ...value, sourceMarkIn: value.sourcePlayheadTicks }
              : { ...value, programMarkIn: value.playheadTicks },
          );
          return;
        case "mark-out":
          setSession((value) =>
            value.focusedMonitor === "source"
              ? { ...value, sourceMarkOut: value.sourcePlayheadTicks }
              : { ...value, programMarkOut: value.playheadTicks },
          );
          return;
        case "trim-previous":
          if (sequenceId) execute(commands.trimPrevious(sequenceId, current.playheadTicks));
          return;
        case "trim-next":
          if (sequenceId) execute(commands.trimNext(sequenceId, current.playheadTicks));
          return;
        case "step":
          setSession((value) => stepFrame(value, projectRef.current, action.direction));
          return;
        case "ripple-delete":
          if (sequenceId && current.selectedClipIds.length > 0) {
            execute(commands.rippleDelete(sequenceId, current.selectedClipIds));
            setSession((value) => ({ ...value, selectedClipIds: [] }));
          }
          return;
        default: {
          const unknown: never = action;
          throw new Error(`未対応の操作 ${String(unknown)}`);
        }
      }
    },
    [execute, redo, save, undo],
  );

  const applyRef = useRef(applyAction);
  applyRef.current = applyAction;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target;
      if (target instanceof HTMLElement) {
        const tag = target.tagName;
        if (tag === "INPUT" || tag === "TEXTAREA" || target.isContentEditable) return;
      }
      const action = actionFromKey({
        key: event.key,
        ctrlKey: event.ctrlKey,
        metaKey: event.metaKey,
        shiftKey: event.shiftKey,
        altKey: event.altKey,
      });
      if (!action) return;
      event.preventDefault();
      applyRef.current(action);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    const playing = session.playing;
    if (!playing) return;
    let last = performance.now();
    let handle = 0;
    const step = (now: number) => {
      const dt = Math.min(0.1, Math.max(0, (now - last) / 1000));
      last = now;
      const delta = Math.round(dt * TICKS_PER_SECOND * playing.speed) * playing.direction;
      setSession((current) => movePlayhead(current, projectRef.current, delta));
      handle = requestAnimationFrame(step);
    };
    handle = requestAnimationFrame(step);
    return () => cancelAnimationFrame(handle);
  }, [session.playing]);

  useEffect(() => {
    const title = `VideoCut — ${project.name}${stack.dirty ? " *" : ""}`;
    void setWindowTitle(title).catch(() => {
      document.title = title;
    });
  }, [project.name, revision, stack]);

  const razorAtPlayhead = useCallback(() => {
    const current = sessionRef.current;
    if (!current.activeSequenceId) return;
    const ticks = current.playheadTicks;
    if (!execute(commands.razor(current.activeSequenceId, ticks))) return;
    const sequence = sequenceById(stack.project, current.activeSequenceId);
    setSession((value) => ({
      ...value,
      selectedClipIds: sequence ? clipsStartingAt(sequence, ticks) : [],
    }));
  }, [execute, stack]);

  const overwriteSource = useCallback(() => {
    const current = sessionRef.current;
    const documentProject = stack.project;
    const sequence = sequenceById(documentProject, current.activeSequenceId);
    if (!sequence || !current.activeSequenceId) return;
    const frame = ticksPerFrame(sequence.settings.fps);
    if (current.sourceMediaId) {
      const media = documentProject.media.find((item) => item.id === current.sourceMediaId);
      if (!media) return;
      const duration = media.durationTicks > 0 ? media.durationTicks : frame * 150;
      const sourceIn = current.sourceMarkIn ?? 0;
      const sourceOut = current.sourceMarkOut ?? duration;
      const span = Math.max(frame, sourceOut - sourceIn);
      const audioOnly = Boolean(media.audio) && !media.video;
      const both = Boolean(media.audio) && Boolean(media.video);
      execute(
        commands.overwrite({
          sequenceId: sequence.id,
          name: media.name,
          mediaId: media.id,
          kind: "media",
          timelineIn: current.playheadTicks,
          durationTicks: span,
          sourceIn,
          sourceOut: sourceIn + span,
          labelColor: 6,
          video: !audioOnly,
          audio: audioOnly || both,
        }),
      );
      return;
    }
    if (!current.sourceClipId) return;
    const hit = findClip(sequence, current.sourceClipId);
    if (!hit) return;
    const duration = Math.max(frame, hit.clip.sourceOut - hit.clip.sourceIn);
    const sourceIn = current.sourceMarkIn ?? 0;
    const sourceOut = current.sourceMarkOut ?? duration;
    const span = Math.max(frame, Math.min(duration - sourceIn, sourceOut - sourceIn));
    execute(
      commands.overwrite({
        sequenceId: sequence.id,
        name: hit.clip.name,
        ...(hit.clip.mediaId ? { mediaId: hit.clip.mediaId } : {}),
        kind: hit.clip.kind,
        timelineIn: current.playheadTicks,
        durationTicks: span,
        sourceIn: hit.clip.sourceIn + sourceIn,
        sourceOut: hit.clip.sourceIn + sourceIn + span,
        labelColor: hit.clip.labelColor,
        video: hit.track.kind === "video",
        audio: hit.track.kind === "audio",
      }),
    );
  }, [execute, stack]);

  const runDocument = useCallback((action: () => Promise<void>) => {
    void action().catch((error: unknown) => {
      setSession((current) => ({ ...current, message: errorText(error) }));
    });
  }, []);

  return {
    project: shown,
    committed: project,
    session,
    canUndo: stack.canUndo,
    canRedo: stack.canRedo,
    undoLabel: stack.lastUndoLabel,
    redoLabel: stack.lastRedoLabel,
    dirty: stack.dirty,
    execute,
    undo,
    redo,
    previewCommand(command: EditCommand | null) {
      if (!command) {
        setPreview(null);
        return;
      }
      setPreview(command.apply(structuredClone(stack.project)));
    },
    setTool(tool: ToolId) {
      setSession((current) => ({ ...current, tool }));
    },
    setPlayhead(ticks: number) {
      setSession((current) => ({
        ...current,
        playheadTicks: Math.max(0, Math.round(ticks)),
        playing: null,
        focusedMonitor: "program",
      }));
    },
    setSourcePlayhead(ticks: number) {
      setSession((current) => ({
        ...current,
        sourcePlayheadTicks: Math.max(0, Math.round(ticks)),
        playing: null,
        focusedMonitor: "source",
      }));
    },
    setSelected(ids: string[]) {
      setSession((current) => ({ ...current, selectedClipIds: ids }));
    },
    focusMonitor(monitor: "source" | "program") {
      setSession((current) => ({ ...current, focusedMonitor: monitor }));
    },
    togglePlay(monitor: "source" | "program") {
      setSession((current) => {
        if (current.playing?.monitor === monitor) {
          return { ...current, playing: null, focusedMonitor: monitor, message: "停止" };
        }
        return { ...current, focusedMonitor: monitor, playing: { monitor, direction: 1, speed: 1 }, message: "再生" };
      });
    },
    step(monitor: "source" | "program", direction: -1 | 1) {
      setSession((current) => stepFrame({ ...current, focusedMonitor: monitor }, projectRef.current, direction));
    },
    openSourceMedia(mediaId: string) {
      setSession((current) => ({
        ...current,
        sourceMediaId: mediaId,
        sourceClipId: null,
        sourcePlayheadTicks: 0,
        sourceMarkIn: null,
        sourceMarkOut: null,
        focusedMonitor: "source",
        playing: null,
      }));
    },
    openSourceClip(clipId: string) {
      setSession((current) => ({
        ...current,
        sourceClipId: clipId,
        sourceMediaId: null,
        sourcePlayheadTicks: 0,
        sourceMarkIn: null,
        sourceMarkOut: null,
        focusedMonitor: "source",
        playing: null,
      }));
    },
    activateSequence(sequenceId: string) {
      setSession((current) => ({
        ...current,
        activeSequenceId: sequenceId,
        playheadTicks: 0,
        selectedClipIds: [],
        playing: null,
        focusedMonitor: "program",
      }));
    },
    clearMarks() {
      setSession((current) =>
        current.focusedMonitor === "source"
          ? { ...current, sourceMarkIn: null, sourceMarkOut: null }
          : { ...current, programMarkIn: null, programMarkOut: null },
      );
    },
    newProject() {
      replaceDocument(createEmptyProject(), null, "新規プロジェクト");
    },
    open,
    save,
    importMedia,
    openPrproj() {
      void runDocument(async () => {
        const loaded = await loadPrproj();
        if (!loaded) return;
        replaceDocument(loaded.project, null, withWarnings(".prproj を開きました", loaded.warnings));
      });
    },
    savePrproj() {
      void runDocument(async () => {
        const warnings = await storePrproj(stack.project);
        if (!warnings) return;
        setSession((current) => ({ ...current, message: withWarnings(".prproj を保存しました", warnings) }));
      });
    },
    openExchange(format: ExchangeFormat) {
      void runDocument(async () => {
        const loaded = await loadExchange(format);
        if (!loaded) return;
        replaceDocument(loaded.project, null, withWarnings(`${format.toUpperCase()} を開きました`, loaded.warnings));
      });
    },
    saveExchange(format: ExchangeFormat) {
      void runDocument(async () => {
        const warnings = await storeExchange(stack.project, format, sessionRef.current.activeSequenceId);
        if (!warnings) return;
        setSession((current) => ({ ...current, message: withWarnings(`${format.toUpperCase()} を書き出しました`, warnings) }));
      });
    },
    razorAtPlayhead,
    overwriteSource,
    quit() {
      void closeWindow();
    },
    deleteSelection() {
      applyRef.current({ type: "ripple-delete" });
    },
    markIn() {
      applyRef.current({ type: "mark-in" });
    },
    markOut() {
      applyRef.current({ type: "mark-out" });
    },
  };
}

function stepFrame(session: SessionState, project: Project, direction: -1 | 1): SessionState {
  const sequence = sequenceById(project, session.activeSequenceId);
  const quantum = sequence ? ticksPerFrame(sequence.settings.fps) : Math.round(TICKS_PER_SECOND / 30);
  if (session.focusedMonitor === "source") {
    return {
      ...session,
      playing: null,
      sourcePlayheadTicks: Math.max(0, session.sourcePlayheadTicks + direction * quantum),
    };
  }
  return {
    ...session,
    playing: null,
    playheadTicks: Math.max(0, session.playheadTicks + direction * quantum),
  };
}

function errorText(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error) return error;
  return "操作に失敗しました";
}

function withWarnings(message: string, warnings: string[]): string {
  if (warnings.length === 0) return message;
  return `${message}（${warnings.slice(0, 2).join("、")}）`;
}

export type Editor = ReturnType<typeof useEditor>;
