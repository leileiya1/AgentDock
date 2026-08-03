import type { TaskSummary } from "@/generated/bindings";
import { groupForStatus } from "@/copy/status";
import { isTaskExecuting } from "@/lib/taskStatus";

export type HomeView = "all" | "attention" | "running" | "done";

export interface HomeTaskSections {
  all: TaskSummary[];
  attention: TaskSummary[];
  running: TaskSummary[];
  open: TaskSummary[];
  done: TaskSummary[];
  counts: Record<HomeView, number>;
}

const newestFirst = (a: TaskSummary, b: TaskSummary) =>
  Date.parse(b.updatedAt) - Date.parse(a.updatedAt) || b.seq - a.seq;

export function buildHomeTaskSections(tasks: TaskSummary[]): HomeTaskSections {
  const all = [...tasks].sort(newestFirst);
  const attention = all.filter((task) => groupForStatus(task.status) === "attention");
  const done = all.filter((task) => groupForStatus(task.status) === "done");
  const open = all.filter((task) => groupForStatus(task.status) === "active");
  const running = open.filter((task) => isTaskExecuting(task.status));

  return {
    all,
    attention,
    running,
    open,
    done,
    counts: {
      all: all.length,
      attention: attention.length,
      running: running.length,
      done: done.length,
    },
  };
}
