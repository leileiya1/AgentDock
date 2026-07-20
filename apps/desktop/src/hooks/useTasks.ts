import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { commands } from "@/generated/bindings";
import type {
  ApproveArgs,
  RejectArgs,
  TaskCreateArgs,
  TaskDetail,
  TaskSummary,
  RepairAction,
} from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";

/** Task list with a 30s safety poll (event-driven updates do the real work). */
export function useTasks(projectId: string | undefined) {
  return useQuery({
    queryKey: qk.tasks(projectId ?? "none"),
    queryFn: () => unwrap(commands.taskList({ projectId: projectId! })),
    enabled: !!projectId,
    refetchInterval: 30_000,
  });
}

export function useTaskDetail(taskId: string | undefined) {
  return useQuery({
    queryKey: qk.task(taskId ?? "none"),
    queryFn: () => unwrap(commands.taskGet({ taskId: taskId! })),
    enabled: !!taskId,
  });
}

/** Reflect a command's returned task into both the detail and list caches. */
export function syncTaskCaches(client: QueryClient, detail: TaskDetail): void {
  client.setQueryData(qk.task(detail.id), detail);
  const summary: TaskSummary = detail;
  client.setQueryData<TaskSummary[]>(qk.tasks(detail.projectId), (prev) => {
    if (!prev) return prev;
    const idx = prev.findIndex((t) => t.id === detail.id);
    if (idx === -1) return [summary, ...prev];
    const next = prev.slice();
    next[idx] = summary;
    return next;
  });
}

function useTaskMutation<A>(fn: (args: A) => Promise<TaskDetail>) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: (detail) => syncTaskCaches(client, detail),
  });
}

export function useCreateTask() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (args: TaskCreateArgs) => unwrap(commands.taskCreate(args)),
    onSuccess: (detail) => {
      syncTaskCaches(client, detail);
      client.invalidateQueries({ queryKey: qk.tasks(detail.projectId) });
    },
  });
}

export const useStartTask = () =>
  useTaskMutation((taskId: string) => unwrap(commands.taskStart({ taskId })));

export const useCancelTask = () =>
  useTaskMutation((taskId: string) => unwrap(commands.taskCancel({ taskId })));

export const useResumeWithGuidance = () =>
  useTaskMutation((args: { taskId: string; guidance: string }) =>
    unwrap(commands.taskResumeWithGuidance(args))
  );

export const useForceApprove = () =>
  useTaskMutation((taskId: string) => unwrap(commands.taskForceApprove({ taskId })));

export const useApproveTask = () =>
  useTaskMutation((args: ApproveArgs) => unwrap(commands.taskApprove(args)));

export const useRejectTask = () =>
  useTaskMutation((args: RejectArgs) => unwrap(commands.taskReject(args)));

export const useMergeTask = () =>
  useTaskMutation((taskId: string) => unwrap(commands.taskMerge({ taskId })));

export const useMarkMergedExternal = () =>
  useTaskMutation((taskId: string) => unwrap(commands.taskMarkMergedExternal({ taskId })));

export function useRepairReport(taskId: string, enabled: boolean) {
  return useQuery({
    queryKey: qk.repair(taskId),
    queryFn: () => unwrap(commands.taskRepairInspect({ taskId })),
    enabled,
  });
}

export const useApplyRepair = () =>
  useTaskMutation((args: { taskId: string; action: RepairAction }) =>
    unwrap(commands.taskRepairApply(args))
  );
