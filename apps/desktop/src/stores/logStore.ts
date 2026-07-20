import { create } from "zustand";
import type { AgentEvent } from "@/generated/bindings";

/** Ring-buffer cap per run (02 §5). Older lines are dropped from the head. */
export const LOG_RING_CAP = 5000;

interface RunBuffer {
  lines: AgentEvent[];
  /** true once the head has been trimmed (older output exists on disk). */
  headTrimmed: boolean;
  /** next disk line offset to request when loading earlier history. */
  historyFromLine: number;
  /** whether the whole file has been loaded (runLogTail eof). */
  eof: boolean;
}

interface LogState {
  buffers: Record<string, RunBuffer>;
  append: (runId: string, events: AgentEvent[]) => void;
  /** replace buffer with a freshly loaded history page (oldest-first). */
  setHistory: (runId: string, events: AgentEvent[], nextFromLine: number, eof: boolean) => void;
  prependHistory: (
    runId: string,
    events: AgentEvent[],
    nextFromLine: number,
    eof: boolean
  ) => void;
  ensure: (runId: string) => void;
  clear: (runId: string) => void;
}

const emptyBuffer = (): RunBuffer => ({
  lines: [],
  headTrimmed: false,
  historyFromLine: 0,
  eof: false,
});

export const useLogStore = create<LogState>((set) => ({
  buffers: {},

  ensure: (runId) =>
    set((s) =>
      s.buffers[runId] ? s : { buffers: { ...s.buffers, [runId]: emptyBuffer() } }
    ),

  append: (runId, events) =>
    set((s) => {
      const buf = s.buffers[runId] ?? emptyBuffer();
      let lines = buf.lines.concat(events);
      let headTrimmed = buf.headTrimmed;
      if (lines.length > LOG_RING_CAP) {
        lines = lines.slice(lines.length - LOG_RING_CAP);
        headTrimmed = true;
      }
      return { buffers: { ...s.buffers, [runId]: { ...buf, lines, headTrimmed } } };
    }),

  setHistory: (runId, events, nextFromLine, eof) =>
    set((s) => {
      const buf = s.buffers[runId] ?? emptyBuffer();
      let lines = events.slice(-LOG_RING_CAP);
      return {
        buffers: {
          ...s.buffers,
          [runId]: {
            ...buf,
            lines,
            headTrimmed: events.length > lines.length,
            historyFromLine: nextFromLine,
            eof,
          },
        },
      };
    }),

  prependHistory: (runId, events, nextFromLine, eof) =>
    set((s) => {
      const buf = s.buffers[runId] ?? emptyBuffer();
      let lines = events.concat(buf.lines);
      let headTrimmed = buf.headTrimmed;
      if (lines.length > LOG_RING_CAP) {
        lines = lines.slice(0, LOG_RING_CAP);
        headTrimmed = true;
      }
      return {
        buffers: {
          ...s.buffers,
          [runId]: { ...buf, lines, headTrimmed, historyFromLine: nextFromLine, eof },
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
