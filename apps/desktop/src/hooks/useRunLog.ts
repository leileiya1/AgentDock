import { useCallback, useEffect, useRef, useState } from "react";
import { commands } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { useLogStore } from "@/stores/logStore";

const PAGE = 1000;

/**
 * Seeds a run's ring buffer from agent-events.jsonl via runLogTail, forward-
 * paginated. Live runs then keep appending through the run:log stream
 * (useRunLogStream). History for a run is loaded once; callers can loadMore().
 */
export function useRunLog(runId: string | undefined) {
  const ensure = useLogStore((s) => s.ensure);
  const setHistory = useLogStore((s) => s.setHistory);
  const append = useLogStore((s) => s.append);
  const buffer = useLogStore((s) => (runId ? s.buffers[runId] : undefined));

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const seeded = useRef<Set<string>>(new Set());

  const loadPage = useCallback(
    async (rid: string, fromLine: number, replace: boolean) => {
      setLoading(true);
      setError(null);
      try {
        const page = await unwrap(commands.runLogTail({ runId: rid, fromLine, maxLines: PAGE }));
        if (replace) setHistory(rid, page.lines, page.nextFromLine, page.eof);
        else {
          append(rid, page.lines);
          useLogStore.setState((s) => {
            const buf = s.buffers[rid];
            if (!buf) return s;
            return { buffers: { ...s.buffers, [rid]: { ...buf, historyFromLine: page.nextFromLine, eof: page.eof } } };
          });
        }
      } catch (e) {
        setError(e);
      } finally {
        setLoading(false);
      }
    },
    [setHistory, append]
  );

  useEffect(() => {
    if (!runId) return;
    ensure(runId);
    if (seeded.current.has(runId)) return;
    seeded.current.add(runId);
    void loadPage(runId, 0, true);
  }, [runId, ensure, loadPage]);

  const loadMore = useCallback(() => {
    if (!runId || !buffer || buffer.eof || loading) return;
    void loadPage(runId, buffer.historyFromLine, false);
  }, [runId, buffer, loading, loadPage]);

  return { buffer, loading, error, loadMore, hasMore: !!buffer && !buffer.eof };
}
