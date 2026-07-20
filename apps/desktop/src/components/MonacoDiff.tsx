import { useRef } from "react";
import { DiffEditor, type Monaco } from "@monaco-editor/react";
import type { editor } from "monaco-editor";
import { setupMonaco, languageForPath } from "@/lib/monaco";
import { parseUnifiedPatch, reconstructedRowForLine } from "@/lib/diff";

setupMonaco();

interface Props {
  path: string;
  patch: string | null;
  /** real modified-file line to reveal, if any */
  jumpLine?: number | null;
}

/** Read-only Monaco DiffEditor for one file. Lazy-loaded via React.lazy. */
export default function MonacoDiff({ path, patch, jumpLine }: Props) {
  const parsed = parseUnifiedPatch(patch);
  const editorRef = useRef<editor.IStandaloneDiffEditor | null>(null);

  const onMount = (ed: editor.IStandaloneDiffEditor, _monaco: Monaco) => {
    editorRef.current = ed;
    revealJump();
  };

  const revealJump = () => {
    const ed = editorRef.current;
    if (!ed || jumpLine == null) return;
    const row = reconstructedRowForLine(parsed.modifiedLineMap, jumpLine);
    if (row != null) {
      const modified = ed.getModifiedEditor();
      modified.revealLineInCenter(row);
      modified.setPosition({ lineNumber: row, column: 1 });
    }
  };

  // Reveal again if the jump target changes for the same file.
  if (editorRef.current && jumpLine != null) revealJump();

  const language = languageForPath(path);

  return (
    <DiffEditor
      original={parsed.original}
      modified={parsed.modified}
      language={language}
      theme="agentflow-light"
      onMount={onMount}
      options={{
        readOnly: true,
        renderSideBySide: true,
        useInlineViewWhenSpaceIsLimited: true,
        automaticLayout: true,
        minimap: { enabled: false },
        scrollBeyondLastLine: false,
        fontFamily: "IBM Plex Mono, ui-monospace, monospace",
        fontSize: 12,
        lineNumbers: "on",
        renderOverviewRuler: false,
        diffWordWrap: "off",
      }}
    />
  );
}
