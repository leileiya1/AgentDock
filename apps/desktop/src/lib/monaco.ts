import * as monaco from "monaco-editor";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import tsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";
import cssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import htmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import { loader } from "@monaco-editor/react";

/*
 * Monaco is loaded from the bundled local package — never the CDN — so the app
 * works offline (02 §2.2, no runtime网络依赖). Workers are wired through Vite's
 * ?worker imports.
 */
let configured = false;

export function setupMonaco(): void {
  if (configured) return;
  configured = true;

  (self as unknown as { MonacoEnvironment: unknown }).MonacoEnvironment = {
    getWorker(_id: string, label: string) {
      switch (label) {
        case "json":
          return new jsonWorker();
        case "css":
        case "scss":
        case "less":
          return new cssWorker();
        case "html":
        case "handlebars":
        case "razor":
          return new htmlWorker();
        case "typescript":
        case "javascript":
          return new tsWorker();
        default:
          return new editorWorker();
      }
    },
  };

  loader.config({ monaco });

  // A warm light theme aligned with the 米白 + Claude 橙 tokens.
  monaco.editor.defineTheme("agentflow-light", {
    base: "vs",
    inherit: true,
    rules: [],
    colors: {
      "editor.background": "#FBF8F2",
      "editorGutter.background": "#FBF8F2",
      "editor.foreground": "#2B2620",
      "editor.lineHighlightBackground": "#F1EADD",
      "editorLineNumber.foreground": "#9B9184",
      "editorLineNumber.activeForeground": "#6E655A",
      "diffEditor.insertedTextBackground": "#3F966833",
      "diffEditor.removedTextBackground": "#C24A3A2E",
      "diffEditor.insertedLineBackground": "#DCEEDF80",
      "diffEditor.removedLineBackground": "#F6DED980",
      "editorWidget.background": "#FBF8F2",
      "editorWidget.border": "#E7DFD1",
    },
  });
}

/** Guess a Monaco language id from a file path (best-effort, read-only). */
export function languageForPath(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  const map: Record<string, string> = {
    ts: "typescript",
    tsx: "typescript",
    js: "javascript",
    jsx: "javascript",
    mjs: "javascript",
    cjs: "javascript",
    json: "json",
    css: "css",
    scss: "scss",
    less: "less",
    html: "html",
    md: "markdown",
    rs: "rust",
    py: "python",
    go: "go",
    java: "java",
    kt: "kotlin",
    c: "c",
    h: "c",
    cpp: "cpp",
    hpp: "cpp",
    cs: "csharp",
    rb: "ruby",
    php: "php",
    sh: "shell",
    bash: "shell",
    yml: "yaml",
    yaml: "yaml",
    toml: "ini",
    sql: "sql",
    xml: "xml",
  };
  return map[ext] ?? "plaintext";
}
