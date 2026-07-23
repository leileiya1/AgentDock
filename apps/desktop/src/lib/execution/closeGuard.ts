import type { TaskStatus, TaskSummary } from "@/generated/bindings";

/** Statuses where work is genuinely in flight and will keep running in the background daemon. */
const ACTIVE_STATUSES: ReadonlySet<TaskStatus> = new Set<TaskStatus>([
  "PLANNING",
  "DEVELOPING",
  "VALIDATING",
  "REVIEWING",
  "REVISING",
  "MERGING",
]);

/**
 * Count tasks that are actively running, across every cached per-project task list. Used by the
 * §40 close guard to decide whether to inform the user that closing the window leaves work running
 * in the background (rather than nagging on every close when nothing is in flight).
 */
export function countActiveTasks(taskLists: readonly (readonly TaskSummary[] | undefined)[]): number {
  let count = 0;
  for (const list of taskLists) {
    for (const task of list ?? []) {
      if (ACTIVE_STATUSES.has(task.status)) count += 1;
    }
  }
  return count;
}
