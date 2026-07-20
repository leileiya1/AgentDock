import { useMemo } from "react";
import type { RunSummary } from "@/generated/bindings";
import { useReview, useRuns } from "@/hooks/useTaskData";
import { useUiStore } from "@/stores/uiStore";
import { RunLogViewer } from "@/components/RunLogViewer";
import { AgentMark } from "@/components/AgentMark";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";

const ROLE_LABEL: Record<RunSummary["role"], string> = { planner: "计划", developer: "开发", reviewer: "审查", validator: "验证" };
const STATUS_LABEL: Record<RunSummary["status"], string> = {
  RUNNING: "进行中", SUCCEEDED: "成功", FAILED: "失败", TIMED_OUT: "超时", CANCELLED: "已取消", INTERRUPTED: "已中断",
};
const STATUS_DOT: Record<RunSummary["status"], string> = {
  RUNNING: "bg-run animate-pulse-dot",
  SUCCEEDED: "bg-ok",
  FAILED: "bg-bad",
  TIMED_OUT: "bg-bad",
  CANCELLED: "bg-idle",
  INTERRUPTED: "bg-idle",
};

export function LogsTab({ taskId }: { taskId: string }) {
  const runs = useRuns(taskId);
  const selectedRun = useUiStore((s) => s.selectedRun[taskId]);
  const setSelectedRun = useUiStore((s) => s.setSelectedRun);

  const sorted = useMemo(() => {
    return (runs.data ?? []).slice().sort((a, b) => {
      const ta = a.startedAt ? new Date(a.startedAt).getTime() : 0;
      const tb = b.startedAt ? new Date(b.startedAt).getTime() : 0;
      return tb - ta;
    });
  }, [runs.data]);

  const activeRunId = selectedRun ?? sorted[0]?.id;
  const activeRun = sorted.find((run) => run.id === activeRunId);
  const review = useReview(
    taskId,
    activeRun?.role === "reviewer" && activeRun.status === "SUCCEEDED" ? activeRun.revision : undefined,
  );
  const reviewHighlight = useMemo(() => {
    if (!review.data) return undefined;
    const issues = review.data.issues.filter((issue) => !issue.resolved).slice(0, 3);
    const issueText = issues.length > 0
      ? `需要处理：\n${issues.map((issue, index) => `${index + 1}. ${issue.title}`).join("\n")}`
      : "没有发现需要处理的问题。";
    return [review.data.summary, issueText].filter(Boolean).join("\n\n");
  }, [review.data]);

  if (runs.isLoading) return <div className="p-4"><SkeletonRows rows={4} /></div>;
  if (runs.isError) return <ErrorState error={runs.error} onRetry={() => runs.refetch()} />;
  if (sorted.length === 0) return <EmptyState title="还没有运行" hint="任务开始后，这里会列出每一次开发、验证与审查运行。" />;

  return (
    <div className="flex h-full min-h-0">
      <div className="flex w-60 shrink-0 flex-col gap-0.5 overflow-y-auto border-r border-line/70 p-2">
        {sorted.map((r) => (
          <button
            key={r.id}
            onClick={() => setSelectedRun(taskId, r.id)}
            className={cn(
              "flex items-center gap-2 rounded-md border px-2 py-2 text-left transition-colors",
              r.id === activeRunId ? "border-line bg-raised" : "border-transparent hover:bg-raised"
            )}
          >
            <span className={cn("size-2 shrink-0 rounded-full", STATUS_DOT[r.status])} />
            <span className="shrink-0 text-[13px]">
              {ROLE_LABEL[r.role]}
              <span className="font-mono text-[12px] text-t3"> r{r.revision}</span>
            </span>
            {r.agent && <AgentMark kind={r.agent} />}
            <span className="ml-auto text-right text-[11px] text-t3">
              {STATUS_LABEL[r.status]} · {relativeTime(r.startedAt)}
            </span>
          </button>
        ))}
      </div>
      <div className="flex min-w-0 flex-1 flex-col">
        {activeRunId ? (
          <RunLogViewer key={activeRunId} runId={activeRunId} preferredSummary={reviewHighlight} />
        ) : (
          <EmptyState title="选择一个运行查看日志" />
        )}
      </div>
    </div>
  );
}
