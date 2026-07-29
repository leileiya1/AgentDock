import type { QueueTaskState } from "@/generated/bindings";

export const QUEUE_RELEVANT_STATUSES = new Set([
  "DRAFT",
  "PLANNING",
  "READY_FOR_DEVELOPMENT",
  "DEVELOPING",
  "VALIDATING",
  "READY_FOR_REVIEW",
  "REVIEWING",
  "READY_FOR_REVISION",
  "REVISING",
]);

export function queueSummary(queue: QueueTaskState): string {
  if (queue.state === "RUNNING") return "正在运行";
  if (queue.state === "COMPLETED") return "调度已完成";
  if (queue.state === "FAILED") return "自动重试已停止";

  switch (queue.waitingReason) {
    case "paused":
      return "已暂停排队";
    case "scheduler_paused":
      return "全部任务已暂停调度";
    case "outside_run_window":
      return "等待允许运行时段";
    case "daily_budget_exhausted":
      return "今日总预算已用尽";
    case "retry_delay":
      return queue.notBefore
        ? `等待重试（${new Date(queue.notBefore).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}）`
        : "等待重试";
    case "concurrency_limit":
      return "运行槽已满";
    case "tasks_ahead":
      return queue.position ? `队列第 ${queue.position} 位` : "等待前序任务";
    default:
      return queue.position ? `队列第 ${queue.position} 位` : "等待调度";
  }
}

export function canMutateQueue(queue: QueueTaskState | null | undefined): boolean {
  return queue?.state === "QUEUED";
}
