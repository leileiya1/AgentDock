import { useCallback, useEffect, useState } from "react";
import { commands } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { useLogStore } from "@/stores/logStore";

const PAGE = 1000;

/**
 * Seeds a run's ring buffer from agent-events.jsonl and lets the caller page backwards.
 *
 * The seed loads the *most recent* page, not the file's opening lines: a long run's final
 * structured result lives at the end, and seeding from the head would leave a hole between the
 * first page and the live tail. Every write is positioned by absolute line number, so the live
 * stream (useRunLogStream) can overlap the seed without duplicating it, and re-entering the page
 * resumes the existing buffer instead of replacing it.
 */
export function useRunLog(runId: string | undefined) {
  const ensure = useLogStore((s) => s.ensure);
  const merge = useLogStore((s) => s.merge);
  const prependHistory = useLogStore((s) => s.prependHistory);
  const buffer = useLogStore((s) => (runId ? s.buffers[runId] : undefined));

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<unknown>(null);

  useEffect(() => {
    if (!runId) return;
    ensure(runId);
    // Never clobber a buffer that already holds output: the live stream may have been
    // appending to it while this component was unmounted.
    if ((useLogStore.getState().buffers[runId]?.lines.length ?? 0) > 0) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    void (async () => {
      try {
        const page = await unwrap(
          commands.runLogTail({ runId, fromLine: null, maxLines: PAGE })
        );
        if (!cancelled) merge(runId, page.fromLine, page.lines);
      } catch (e) {
        if (!cancelled) setError(e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [runId, ensure, merge]);

  /** Loads the page immediately before what is buffered (scroll-to-top). */
  const loadMore = useCallback(() => {
    if (!runId || !buffer || buffer.atStart || loading || buffer.firstLine === 0) return;
    const from = Math.max(0, buffer.firstLine - PAGE);
    setLoading(true);
    setError(null);
    void (async () => {
      try {
        const page = await unwrap(
          commands.runLogTail({ runId, fromLine: from, maxLines: buffer.firstLine - from })
        );
        prependHistory(runId, page.fromLine, page.lines, page.totalLines);
      } catch (e) {
        setError(e);
      } finally {
        setLoading(false);
      }
    })();
  }, [runId, buffer, loading, prependHistory]);

  return {
    buffer,
    loading,
    error,
    loadMore,
    hasMore: !!buffer && !buffer.atStart && buffer.firstLine > 0,
  };
}
