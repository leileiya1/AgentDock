import { beforeEach, describe, expect, it } from "bun:test";
import type { AgentEvent } from "@/generated/bindings";
import { LOG_RING_CAP, useLogStore } from "@/stores/logStore";

const line = (index: number): AgentEvent => ({
  ts: "2026-07-27T00:00:00Z",
  stream: "stdout",
  kind: "assistant_text",
  summary: `line ${index}`,
  text: null,
});

const page = (from: number, count: number) =>
  Array.from({ length: count }, (_, offset) => line(from + offset));

const summaries = (runId: string) =>
  (useLogStore.getState().buffers[runId]?.lines ?? []).map((event) => event.summary);

describe("logStore line-number reconciliation", () => {
  beforeEach(() => {
    useLogStore.setState({ buffers: {} });
  });

  it("absorbs a live batch that overlaps already-seeded history instead of duplicating it", () => {
    const { merge } = useLogStore.getState();
    // Seeded tail: lines 100..104.
    merge("run", 100, page(100, 5));
    // The bridge streams from an earlier cursor and overlaps the seed.
    merge("run", 102, page(102, 5));
    expect(summaries("run")).toEqual([
      "line 100",
      "line 101",
      "line 102",
      "line 103",
      "line 104",
      "line 105",
      "line 106",
    ]);
    expect(useLogStore.getState().buffers.run?.nextLine).toBe(107);
  });

  it("drops a batch that is entirely behind the buffer", () => {
    const { merge } = useLogStore.getState();
    merge("run", 100, page(100, 5));
    merge("run", 100, page(100, 3));
    expect(summaries("run")).toHaveLength(5);
  });

  it("keeps live output in order across a re-mounted viewer", () => {
    const { merge, ensure } = useLogStore.getState();
    merge("run", 0, page(0, 3));
    // A viewer re-mounting must not reset the buffer to the head of the file; the seed is
    // skipped when output is already held, and later batches simply continue.
    ensure("run");
    merge("run", 3, page(3, 2));
    expect(summaries("run")).toEqual(["line 0", "line 1", "line 2", "line 3", "line 4"]);
  });

  it("prepends an earlier page ahead of the buffer and marks the file start", () => {
    const { merge, prependHistory } = useLogStore.getState();
    merge("run", 10, page(10, 3));
    prependHistory("run", 0, page(0, 10), 13);
    expect(summaries("run")[0]).toBe("line 0");
    expect(summaries("run")).toHaveLength(13);
    expect(useLogStore.getState().buffers.run?.firstLine).toBe(0);
    expect(useLogStore.getState().buffers.run?.atStart).toBe(true);
  });

  it("never lets a prepended page overlap what is already buffered", () => {
    const { merge, prependHistory } = useLogStore.getState();
    merge("run", 5, page(5, 3));
    // An older page that runs past the buffer's start keeps only the part in front of it.
    prependHistory("run", 0, page(0, 8), 8);
    expect(summaries("run")).toEqual([
      "line 0",
      "line 1",
      "line 2",
      "line 3",
      "line 4",
      "line 5",
      "line 6",
      "line 7",
    ]);
  });

  it("trims the oldest lines when appending past the ring cap and tracks the new start", () => {
    const { merge } = useLogStore.getState();
    merge("run", 0, page(0, LOG_RING_CAP));
    merge("run", LOG_RING_CAP, page(LOG_RING_CAP, 10));
    const buffer = useLogStore.getState().buffers.run;
    expect(buffer?.lines).toHaveLength(LOG_RING_CAP);
    expect(buffer?.firstLine).toBe(10);
    expect(buffer?.nextLine).toBe(LOG_RING_CAP + 10);
    expect(buffer?.headTrimmed).toBe(true);
    expect(buffer?.lines[0]?.summary).toBe("line 10");
  });

  it("keeps the requested older lines when a prepend exceeds the ring cap", () => {
    const { merge, prependHistory } = useLogStore.getState();
    merge("run", LOG_RING_CAP, page(LOG_RING_CAP, 100));
    prependHistory("run", 0, page(0, LOG_RING_CAP), LOG_RING_CAP + 100);
    const buffer = useLogStore.getState().buffers.run;
    // Scrolling backwards must keep what the user just asked for, not discard it.
    expect(buffer?.lines).toHaveLength(LOG_RING_CAP);
    expect(buffer?.firstLine).toBe(0);
    expect(buffer?.lines[0]?.summary).toBe("line 0");
  });
});
