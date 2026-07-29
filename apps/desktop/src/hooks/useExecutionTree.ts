import { useMemo } from "react";
import type { TaskDetail } from "@/generated/bindings";
import { normalizeEvents } from "@/lib/execution/normalize";
import { buildExecutionTree, type ExecutionTree } from "@/lib/execution/tree";
import { liveStatus, type LiveStatus } from "@/lib/execution/liveStatus";
import { useEvents, useRuns } from "@/hooks/useTaskData";
import { useLogStore } from "@/stores/logStore";

/**
 * Assembles the execution tree from the two backend sources (events + runs) and the
 * live log buffer. Kept thin on purpose: every derivation lives in `lib/execution/*`
 * so it stays testable without React (05 §10).
 */
export function useExecutionTree(task: TaskDetail | undefined) {
  const events = useEvents(task?.id);
  const runs = useRuns(task?.id);
  const buffers = useLogStore((s) => s.buffers);

  const tree: ExecutionTree | null = useMemo(() => {
    if (!task) return null;
    return buildExecutionTree({
      events: normalizeEvents(events.data ?? []),
      runs: runs.data ?? [],
      currentRevision: task.currentRevision,
      status: task.status,
      requirePlanApproval: task.policy.requirePlanApproval ?? false,
    });
  }, [task, events.data, runs.data]);

  /**
   * "仍在运行" must be decided by real activity, so take the newest of: the last task
   * event, the last run transition, and the last streamed log line of a running run.
   */
  const lastActivityAt = useMemo(() => {
    const stamps: string[] = [];
    const lastEvent = (events.data ?? []).at(-1);
    if (lastEvent) stamps.push(lastEvent.createdAt);
    for (const run of runs.data ?? []) {
      if (run.finishedAt) stamps.push(run.finishedAt);
      else if (run.startedAt) stamps.push(run.startedAt);
      if (run.status !== "RUNNING") continue;
      const line = buffers[run.id]?.lines.at(-1);
      if (line?.ts) stamps.push(line.ts);
    }
    return stamps.sort().at(-1) ?? null;
  }, [events.data, runs.data, buffers]);

  const live: LiveStatus | null = useMemo(() => {
    if (!tree || !task) return null;
    return liveStatus({ tree, status: task.status, lastActivityAt });
  }, [tree, task, lastActivityAt]);

  return {
    tree,
    live,
    lastActivityAt,
    /** Raw rows, for views that still read payloads directly (概览). */
    events: events.data ?? [],
    runs: runs.data ?? [],
    isLoading: events.isLoading || runs.isLoading,
    error: events.error ?? runs.error,
    refetch: () => {
      void events.refetch();
      void runs.refetch();
    },
  };
}
