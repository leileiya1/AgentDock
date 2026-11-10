import { useQueries } from "@tanstack/react-query";
import type { TaskSummary } from "@/generated/bindings";
import { commands } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";
import { useProjects } from "@/hooks/useProjects";

/** 所有项目的任务摊平成一个数组（共用每个项目的 task 缓存）。`key` 在任一项目刷新时变化。 */
export function useAllTasks(enabled = true): { tasks: TaskSummary[]; key: string; ready: boolean } {
  const projects = useProjects();
  const queries = useQueries({
    queries: (projects.data ?? []).map((p) => ({
      queryKey: qk.tasks(p.id),
      queryFn: () => unwrap(commands.taskList({ projectId: p.id })),
      enabled,
      staleTime: 30_000,
    })),
  });
  return {
    tasks: queries.flatMap((q) => q.data ?? []),
    key: queries.map((q) => q.dataUpdatedAt).join(","),
    ready: projects.isSuccess && queries.every((q) => !q.isPending),
  };
}
