import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type QueueTaskState, type TaskDetail } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";

/**
 * 队列控制入口 (05 §6.7). The daemon has exposed pause/resume/priority since v1,
 * but the UI had no way to reach them — P0-06.
 */
export function useQueueState(taskId: string, enabled: boolean) {
  return useQuery({
    queryKey: qk.queue(taskId),
    queryFn: () => unwrap(commands.queueTaskStatus({ taskId })),
    enabled,
    // Queue rows change independently of TaskDetail while the scheduler claims
    // work. Events refresh quickly; this poll is the restart/reconnect safety net.
    refetchInterval: enabled ? 5_000 : false,
  });
}

function useQueueMutation(fn: (taskId: string, priority: number) => Promise<QueueTaskState>) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ taskId, priority = 0 }: { taskId: string; priority?: number }) => fn(taskId, priority),
    onSuccess: (result) => {
      // Every mutation returns the complete authoritative queue row. Updating the
      // query cache makes the UI instant while remaining restart-safe.
      client.setQueryData(qk.queue(result.taskId), result);
      client.setQueryData<TaskDetail>(qk.task(result.taskId), (prev) =>
        prev ? { ...prev, policy: { ...prev.policy, priority: result.priority } } : prev
      );
      client.invalidateQueries({ queryKey: qk.task(result.taskId) });
    },
  });
}

export const useQueuePause = () =>
  useQueueMutation((taskId) => unwrap(commands.queueTaskPause({ taskId })));

export const useQueueResume = () =>
  useQueueMutation((taskId) => unwrap(commands.queueTaskResume({ taskId })));

export const useQueuePriority = () =>
  useQueueMutation((taskId, priority) => unwrap(commands.queueTaskPriority({ taskId, priority })));

/** 调度优先级 -100…100，给用户三档可理解的选择而不是裸数字 (05 §6.7)。 */
export const PRIORITY_CHOICES: Array<{ value: number; label: string; hint: string }> = [
  { value: 50, label: "优先", hint: "排到其它任务前面" },
  { value: 0, label: "正常", hint: "按创建顺序排队" },
  { value: -50, label: "后台", hint: "空闲时才运行" },
];

export function priorityLabel(priority: number | undefined): string {
  if (priority == null) return "正常";
  const exact = PRIORITY_CHOICES.find((choice) => choice.value === priority);
  if (exact) return exact.label;
  return priority > 0 ? "优先" : priority < 0 ? "后台" : "正常";
}
