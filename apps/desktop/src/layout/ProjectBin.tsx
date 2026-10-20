import { chooseMediaFiles, probeMediaFile } from "../host/desktop-bridge";
import { projectCommand } from "../host/document";
import type { Editor } from "../host/use-editor";
import { ProjectPanel, type MediaImporter } from "../ui/modules";

export function ProjectBin({ editor }: { editor: Editor }) {
  const importer: MediaImporter = {
    async importPaths(paths) {
      const imported = [];
      for (const filePath of paths) imported.push(await probeMediaFile(filePath));
      return imported;
    },
  };
  return (
    <section className="panel bin" data-testid="project-bin">
      <ProjectPanel
        project={editor.project}
        onProjectChange={(project) => editor.execute(projectCommand("プロジェクト", () => project))}
        importer={importer}
        onPickFiles={async () => {
          const files = await chooseMediaFiles();
          return files?.map((file) => file.path) ?? [];
        }}
        onOpenSequence={(sequenceId) => editor.activateSequence(sequenceId)}
        onOpenClip={(mediaId) => editor.openSourceMedia(mediaId)}
      />
    </section>
  );
}
