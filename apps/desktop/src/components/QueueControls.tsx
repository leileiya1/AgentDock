import { Pause, Play } from "lucide-react";
import type { TaskDetail } from "@/generated/bindings";
import {
  PRIORITY_CHOICES,
  priorityLabel,
  useQueuePause,
  useQueuePriority,
  useQueueResume,
  useQueueState,
} from "@/hooks/useQueue";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { canMutateQueue, queueSummary, QUEUE_RELEVANT_STATUSES } from "@/lib/queue";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

/**
 * 任务级排队操作 (05 §6.7): 暂停排队 / 恢复排队 / 调整优先级。
 * 文案强调「暂停的是排队，不是杀掉正在跑的 Agent」——这正是 P0-06 想避免的误解。
 */
export function QueueControls({ task }: { task: TaskDetail }) {
  const relevant = QUEUE_RELEVANT_STATUSES.has(task.status);
  const queue = useQueueState(task.id, relevant);
  const pause = useQueuePause();
  const resume = useQueueResume();
  const priority = useQueuePriority();

  if (!relevant) return null;
  if (queue.isLoading) {
    return <span className="text-meta text-t3">正在读取队列…</span>;
  }
  if (queue.isError) {
    return (
      <Button variant="ghost" size="sm" onClick={() => queue.refetch()}>
        队列状态读取失败 · 重试
      </Button>
    );
  }
  if (!queue.data) return null;

  const paused = queue.data.paused;
  const currentPriority = queue.data.priority;
  const mutable = canMutateQueue(queue.data);
  const busy = pause.isPending || resume.isPending || priority.isPending;

  const run = async (fn: () => Promise<unknown>, done: string) => {
    try {
      await fn();
      toast.info(done);
    } catch (error) {
      toast.error(errorLine(error));
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-2">
      <span
        className="inline-flex items-center gap-1.5 whitespace-nowrap text-meta text-t3"
        title={`队列状态最后更新：${new Date(queue.data.updatedAt).toLocaleString("zh-CN")}`}
      >
        <span
          aria-hidden="true"
          className={`size-1.5 rounded-circle ${queue.data.state === "RUNNING" ? "bg-ok" : queue.data.state === "FAILED" ? "bg-bad" : paused ? "bg-status-human" : "bg-status-running"}`}
        />
        {queueSummary(queue.data)}
      </span>

      {mutable && paused ? (
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => run(() => resume.mutateAsync({ taskId: task.id }), "已恢复排队，轮到时会自动开始")}
        >
          <Play className="size-3.5" /> 恢复排队
        </Button>
      ) : mutable ? (
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() =>
            run(() => pause.mutateAsync({ taskId: task.id }), "已暂停排队；正在运行的 Agent 会跑完当前这一步")
          }
        >
          <Pause className="size-3.5" /> 暂停排队
        </Button>
      ) : null}

      <Select
        value={String(currentPriority)}
        disabled={!mutable || busy}
        onValueChange={(value) =>
          run(
            () => priority.mutateAsync({ taskId: task.id, priority: Number(value) }),
            `优先级已改为「${priorityLabel(Number(value))}」`
          )
        }
      >
        <SelectTrigger className="h-7 w-auto gap-1 px-2 text-meta" aria-label="调整排队优先级">
          <SelectValue placeholder="优先级">{priorityLabel(currentPriority)}</SelectValue>
        </SelectTrigger>
        <SelectContent>
          {PRIORITY_CHOICES.map((choice) => (
            <SelectItem key={choice.value} value={String(choice.value)}>
              {choice.label} · {choice.hint}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
