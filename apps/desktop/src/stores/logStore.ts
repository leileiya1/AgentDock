import { create } from "zustand";
import type { AgentEvent } from "@/generated/bindings";

/** Ring-buffer cap per run (02 §5). Older lines are dropped from the head. */
export const LOG_RING_CAP = 5000;

interface RunBuffer {
  lines: AgentEvent[];
  /** true once the head has been trimmed (older output exists on disk). */
  headTrimmed: boolean;
  /**
   * Absolute file line number of `lines[0]`. Every write is positioned against this, so a
   * re-mount, an overlapping live batch and a history page can never duplicate or reorder
   * output — the three writers previously had no common coordinate at all.
   */
  firstLine: number;
  /** absolute line number just past `lines[lines.length - 1]`. */
  nextLine: number;
  /** whether the oldest line on disk has been loaded (nothing earlier remains). */
  atStart: boolean;
  /** total lines known to exist on disk at the last history load. */
  totalLines: number;
}

interface LogState {
  buffers: Record<string, RunBuffer>;
  /**
   * Applies a positioned batch. Lines already held are skipped, so a live batch that overlaps
   * the seeded history is absorbed rather than duplicated; a batch that starts beyond the
   * buffer's end is appended and the gap recorded by moving `firstLine` only when trimming.
   */
  merge: (runId: string, fromLine: number, events: AgentEvent[]) => void;
  /** Installs an earlier history page ahead of what is already buffered. */
  prependHistory: (runId: string, fromLine: number, events: AgentEvent[], totalLines: number) => void;
  ensure: (runId: string) => void;
  clear: (runId: string) => void;
}

const emptyBuffer = (): RunBuffer => ({
  lines: [],
  headTrimmed: false,
  firstLine: 0,
  nextLine: 0,
  atStart: false,
  totalLines: 0,
});

export const useLogStore = create<LogState>((set) => ({
  buffers: {},

  ensure: (runId) =>
    set((s) => (s.buffers[runId] ? s : { buffers: { ...s.buffers, [runId]: emptyBuffer() } })),

  merge: (runId, fromLine, events) =>
    set((s) => {
      const buf = s.buffers[runId] ?? emptyBuffer();
      if (events.length === 0) return s;
      const end = fromLine + events.length;
      // Entirely behind what we already hold: a duplicate replay, drop it.
      if (buf.lines.length > 0 && end <= buf.nextLine) return s;

      let incoming = events;
      let start = fromLine;
      if (buf.lines.length > 0 && fromLine < buf.nextLine) {
        // Partial overlap — keep only the part we are missing.
        incoming = events.slice(buf.nextLine - fromLine);
        start = buf.nextLine;
      }
      if (incoming.length === 0) return s;

      const empty = buf.lines.length === 0;
      let lines = empty ? [...incoming] : buf.lines.concat(incoming);
      let firstLine = empty ? start : buf.firstLine;
      let headTrimmed = buf.headTrimmed;
      let atStart = empty ? start === 0 : buf.atStart;
      if (lines.length > LOG_RING_CAP) {
        const dropped = lines.length - LOG_RING_CAP;
        lines = lines.slice(dropped);
        firstLine += dropped;
        headTrimmed = true;
        atStart = false;
      }
      const nextLine = start + incoming.length;
      return {
        buffers: {
          ...s.buffers,
          [runId]: {
            ...buf,
            lines,
            headTrimmed,
            firstLine,
            nextLine,
            atStart,
            totalLines: Math.max(buf.totalLines, nextLine),
          },
        },
      };
    }),

  prependHistory: (runId, fromLine, events, totalLines) =>
    set((s) => {
      const buf = s.buffers[runId] ?? emptyBuffer();
      if (events.length === 0) {
        return {
          buffers: { ...s.buffers, [runId]: { ...buf, atStart: true, totalLines } },
        };
      }
      // Only the part strictly before the buffer belongs in front of it.
      const keep = buf.lines.length === 0 ? events : events.slice(0, Math.max(0, buf.firstLine - fromLine));
      if (keep.length === 0) {
        return { buffers: { ...s.buffers, [runId]: { ...buf, atStart: fromLine === 0, totalLines } } };
      }
      let lines = keep.concat(buf.lines);
      const firstLine = fromLine;
      let headTrimmed = buf.headTrimmed;
      let nextLine = buf.lines.length === 0 ? fromLine + keep.length : buf.nextLine;
      if (lines.length > LOG_RING_CAP) {
        // Trim the newest end here: the user is scrolling backwards, so the head they just
        // requested is what must survive.
        lines = lines.slice(0, LOG_RING_CAP);
        nextLine = firstLine + LOG_RING_CAP;
        headTrimmed = true;
      }
      return {
        buffers: {
          ...s.buffers,
          [runId]: {
            ...buf,
            lines,
            headTrimmed,
            firstLine,
            nextLine,
            atStart: fromLine === 0,
            totalLines: Math.max(totalLines, nextLine),
          },
        },
      };
    }),

  clear: (runId) =>
    set((s) => {
      const next = { ...s.buffers };
      delete next[runId];
      return { buffers: next };
    }),
}));
