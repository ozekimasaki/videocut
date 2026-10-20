import type { Project } from "@videocut/model";

export interface EditCommand {
  label: string;
  apply: (project: Project) => Project;
}

const HISTORY_LIMIT = 100;

export class CommandStack {
  private past: Project[] = [];
  private future: Project[] = [];
  private generation = 0;
  private savedGeneration = 0;
  private undoLabel: string | null = null;
  private redoLabel: string | null = null;

  constructor(private current: Project) {}

  get project(): Project {
    return this.current;
  }

  get canUndo(): boolean {
    return this.past.length > 0;
  }

  get canRedo(): boolean {
    return this.future.length > 0;
  }

  get lastUndoLabel(): string | null {
    return this.undoLabel;
  }

  get lastRedoLabel(): string | null {
    return this.redoLabel;
  }

  get dirty(): boolean {
    return this.generation !== this.savedGeneration;
  }

  execute(command: EditCommand): boolean {
    const next = command.apply(structuredClone(this.current));
    if (sameProject(next, this.current)) return false;
    this.past.push(this.current);
    if (this.past.length > HISTORY_LIMIT) this.past.shift();
    this.future = [];
    this.current = next;
    this.generation += 1;
    this.undoLabel = command.label;
    this.redoLabel = null;
    return true;
  }

  undo(): boolean {
    const previous = this.past.pop();
    if (!previous) return false;
    this.future.push(this.current);
    this.redoLabel = this.undoLabel;
    this.current = previous;
    this.generation -= 1;
    this.undoLabel = null;
    return true;
  }

  redo(): boolean {
    const next = this.future.pop();
    if (!next) return false;
    this.past.push(this.current);
    this.undoLabel = this.redoLabel;
    this.current = next;
    this.generation += 1;
    this.redoLabel = null;
    return true;
  }

  markSaved(): void {
    this.savedGeneration = this.generation;
  }

  reset(project: Project): void {
    this.current = project;
    this.past = [];
    this.future = [];
    this.generation = 0;
    this.savedGeneration = 0;
    this.undoLabel = null;
    this.redoLabel = null;
  }
}

function sameProject(left: Project, right: Project): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}
