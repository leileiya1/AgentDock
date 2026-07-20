/**
 * Minimal unified-diff parser. The backend gives us a per-file `patch`; Monaco's
 * DiffEditor wants full original/modified text, so we reconstruct the changed
 * regions from the hunks (read-only view of what changed). Line maps let issue
 * `file:line` jumps land on the right reconstructed row.
 */
export interface ParsedPatch {
  original: string;
  modified: string;
  /** modifiedLineMap[i] = real modified-file line number for reconstructed row i (1-based) */
  modifiedLineMap: number[];
  /** originalLineMap[i] = real original-file line number for reconstructed row i */
  originalLineMap: number[];
  empty: boolean;
}

const HUNK_RE = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;

export function parseUnifiedPatch(patch: string | null | undefined): ParsedPatch {
  const original: string[] = [];
  const modified: string[] = [];
  const originalLineMap: number[] = [];
  const modifiedLineMap: number[] = [];

  if (!patch) {
    return { original: "", modified: "", originalLineMap: [], modifiedLineMap: [], empty: true };
  }

  const lines = patch.split("\n");
  let oldLine = 0;
  let newLine = 0;
  let inHunk = false;
  let firstHunk = true;

  for (const raw of lines) {
    const m = HUNK_RE.exec(raw);
    if (m) {
      oldLine = parseInt(m[1], 10);
      newLine = parseInt(m[3], 10);
      inHunk = true;
      if (!firstHunk) {
        // Visual separator between hunks on both sides.
        original.push("⋯");
        originalLineMap.push(-1);
        modified.push("⋯");
        modifiedLineMap.push(-1);
      }
      firstHunk = false;
      continue;
    }
    if (!inHunk) continue; // skip diff/index/--- /+++ headers
    const tag = raw[0];
    const content = raw.slice(1);
    if (tag === " ") {
      original.push(content);
      originalLineMap.push(oldLine++);
      modified.push(content);
      modifiedLineMap.push(newLine++);
    } else if (tag === "-") {
      original.push(content);
      originalLineMap.push(oldLine++);
    } else if (tag === "+") {
      modified.push(content);
      modifiedLineMap.push(newLine++);
    } else if (raw === "\\ No newline at end of file") {
      // ignore
    }
  }

  return {
    original: original.join("\n"),
    modified: modified.join("\n"),
    originalLineMap,
    modifiedLineMap,
    empty: original.length === 0 && modified.length === 0,
  };
}

/** Map a real modified-file line to the reconstructed row (1-based) for reveal. */
export function reconstructedRowForLine(map: number[], realLine: number): number | null {
  const idx = map.indexOf(realLine);
  if (idx >= 0) return idx + 1;
  // nearest not-exceeding
  let best = -1;
  for (let i = 0; i < map.length; i++) {
    if (map[i] > 0 && map[i] <= realLine) best = i;
  }
  return best >= 0 ? best + 1 : null;
}
