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
  const append = useLogStore((s) => s.append);
  const pending = useRef<Map<string, AgentEvent[]>>(new Map());
  const raf = useRef<number | null>(null);

  const flush = () => {
    raf.current = null;
    const batch = pending.current;
    pending.current = new Map();
    for (const [runId, events] of batch) {
      append(runId, events);
    }
  };

  useTauriEvent("run:log", ({ runId, batch }) => {
    const list = pending.current.get(runId);
    if (list) list.push(...batch);
    else pending.current.set(runId, [...batch]);
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
