import { useEffect, useRef } from "react";
import type { AgentEvent } from "@/generated/bindings";
import { useTauriEvent } from "@/lib/tauriEvents";
import { useLogStore } from "@/stores/logStore";

/**
 * Subscribes to `run:log` and flushes batches into the log ring buffer, frame-
 * coalesced (rAF) on top of the backend's ≤10 batch/s aggregation (02 §5/§7).
 * Mounted at the detail-page level so buffering continues while the logs tab is
 * hidden; the viewer only handles rendering.
 */
export function useRunLogStream(): void {
  const merge = useLogStore((s) => s.merge);
  // Coalesced batches keep their absolute start line so the store can position them against
  // seeded history; consecutive batches for one run are contiguous, so the earliest start wins.
  const pending = useRef<Map<string, { fromLine: number; events: AgentEvent[] }>>(new Map());
  const raf = useRef<number | null>(null);

  const flush = () => {
    raf.current = null;
    const batch = pending.current;
    pending.current = new Map();
    for (const [runId, { fromLine, events }] of batch) {
      merge(runId, fromLine, events);
    }
  };

  useTauriEvent("run:log", ({ runId, fromLine, batch }) => {
    const held = pending.current.get(runId);
    if (held) held.events.push(...batch);
    else pending.current.set(runId, { fromLine, events: [...batch] });
    if (raf.current == null) {
      raf.current = requestAnimationFrame(flush);
    }
  });

  useEffect(() => {
    return () => {
      if (raf.current != null) cancelAnimationFrame(raf.current);
    };
  }, []);
}
