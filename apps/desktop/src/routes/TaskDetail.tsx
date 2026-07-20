import { useEffect, useMemo } from "react";
import { Link, useParams } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { ChevronLeft } from "lucide-react";
import type { DetailTab } from "@/stores/uiStore";
import { useTaskDetail } from "@/hooks/useTasks";
import { useUiStore } from "@/stores/uiStore";
import { useRunLogStream } from "@/hooks/useRunLogStream";
import { useEvents } from "@/hooks/useTaskData";
import { taskCode, shortSha } from "@/lib/format";
import { cn } from "@/lib/utils";
import { agentLabel } from "@/copy/agents";
import { StateBadge } from "@/components/StateBadge";
import { Timeline } from "@/components/Timeline";
import { ApprovalBar } from "@/components/ApprovalBar";
import { AgentMark } from "@/components/AgentMark";
import { CopyText } from "@/components/CopyText";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { OverviewTab } from "@/routes/detail/OverviewTab";
import { LogsTab } from "@/routes/detail/LogsTab";
import { DiffTab } from "@/routes/detail/DiffTab";
import { ReviewTab } from "@/routes/detail/ReviewTab";
import { GovernanceTab } from "@/routes/detail/GovernanceTab";

const TABS: Array<{ id: DetailTab; label: string; key: string }> = [
  { id: "overview", label: "概览", key: "1" },
  { id: "logs", label: "日志", key: "2" },
  { id: "diff", label: "Diff", key: "3" },
  { id: "review", label: "审查", key: "4" },
  { id: "governance", label: "治理", key: "5" },
];

const ACTIVE_STATUSES = new Set(["PLANNING", "DEVELOPING", "VALIDATING", "REVIEWING", "REVISING", "MERGING"]);

export function TaskDetail() {
  const { taskId, projectId } = useParams();
  const task = useTaskDetail(taskId);
  const events = useEvents(taskId);

  const activeTab = useUiStore((s) => (taskId ? s.activeTab[taskId] : undefined)) ?? "overview";
  const setActiveTab = useUiStore((s) => s.setActiveTab);
  const selectedRevStore = useUiStore((s) => (taskId ? s.selectedRevision[taskId] : undefined));
  const setSelectedRevision = useUiStore((s) => s.setSelectedRevision);

  useRunLogStream();

  const detail = task.data;
  const selectedRevision = selectedRevStore ?? detail?.currentRevision ?? 1;

  useEffect(() => {
    if (!taskId) return;
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || (e.target as HTMLElement)?.isContentEditable) return;
      const hit = TABS.find((t) => t.key === e.key);
      if (hit) setActiveTab(taskId, hit.id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [taskId, setActiveTab]);

  useEffect(() => {
    if (!detail || !ACTIVE_STATUSES.has(detail.status)) return;
    const timer = setTimeout(() => {
      task.refetch();
      events.refetch();
    }, 60_000);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detail?.updatedAt, detail?.status]);

  const revisions = useMemo(() => detail?.revisions ?? [], [detail]);

  if (task.isLoading) {
    return (
      <div className="p-6">
        <SkeletonRows rows={6} />
      </div>
    );
  }
  if (task.isError || !detail) {
    return (
      <div className="grid h-full place-items-center p-12">
        <ErrorState error={task.error} onRetry={() => task.refetch()} />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex shrink-0 flex-col gap-2 border-b border-line/70 px-6 py-3">
        <div className="flex min-w-0 items-center gap-3">
          <Link
            to={`/p/${projectId}`}
            className="flex h-7 items-center gap-1 rounded-md px-2 text-[13px] text-t3 transition-colors hover:bg-raised hover:text-t1"
            title="返回任务列表"
          >
            <ChevronLeft className="size-4" /> 返回
          </Link>
          <span className="shrink-0 font-mono text-[13px] text-t3">{taskCode(detail.seq)}</span>
          <h1 className="min-w-0 flex-1 truncate text-[16px] font-semibold">{detail.title}</h1>
          <StateBadge status={detail.status} />
        </div>
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div className="flex gap-1">
            {revisions.map((r) => {
              const on = r.revision === selectedRevision;
              return (
                <button
                  key={r.revision}
                  onClick={() => taskId && setSelectedRevision(taskId, r.revision)}
                  title={r.commitSha ? `commit ${shortSha(r.commitSha)}` : undefined}
                  className={cn(
                    "relative flex items-center gap-1 rounded-md border px-2 py-1 font-mono text-[12px] transition-colors",
                    on ? "border-run/60 bg-raised text-t1" : "border-line text-t2 hover:border-line-strong hover:text-t1"
                  )}
                >
                  r{r.revision}
                  {r.revision === detail.currentRevision && (
                    <span className="font-sans text-[10px] text-t3">当前</span>
                  )}
                </button>
              );
            })}
          </div>
          <div className="flex flex-wrap items-center gap-3 font-mono text-[12px] text-t3">
            {detail.branch && (
              <span className="flex items-center gap-1">分支 <CopyText value={detail.branch} className="text-t2">{detail.branch}</CopyText></span>
            )}
            {detail.baseCommit && (
              <span className="flex items-center gap-1">base <CopyText value={detail.baseCommit} className="text-t2">{shortSha(detail.baseCommit)}</CopyText></span>
            )}
            <span className="flex items-center gap-1.5 font-sans">
              <AgentMark kind={detail.developerAgent} /> 开发 {agentLabel(detail.developerAgent)}
            </span>
            <span className="flex items-center gap-1.5 font-sans">
              <AgentMark kind={detail.reviewerAgent} /> 审查 {agentLabel(detail.reviewerAgent)}
            </span>
          </div>
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        <aside className="w-60 shrink-0 overflow-y-auto border-r border-line/70 px-2 py-2">
          {events.isLoading ? (
            <SkeletonRows rows={5} gap={14} />
          ) : events.isError ? (
            <ErrorState error={events.error} onRetry={() => events.refetch()} compact />
          ) : (
            <Timeline
              events={events.data ?? []}
              developerAgent={detail.developerAgent}
              reviewerAgent={detail.reviewerAgent}
              selectedRevision={selectedRevision}
              onSelectRevision={(rev) => taskId && setSelectedRevision(taskId, rev)}
            />
          )}
        </aside>

        <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
          <div className="flex shrink-0 gap-1 border-b border-line/70 px-4 pt-2" role="tablist" aria-label="任务详情">
            {TABS.map((t) => {
              const on = activeTab === t.id;
              return (
                <button
                  key={t.id}
                  role="tab"
                  aria-selected={on}
                  onClick={() => taskId && setActiveTab(taskId, t.id)}
                  title={`${t.label} (${t.key})`}
                  className={cn(
                    "relative px-3 py-2 text-[13px] font-medium transition-colors",
                    on ? "text-t1" : "text-t2 hover:text-t1"
                  )}
                >
                  {t.label}
                  {on && (
                    <motion.span
                      layoutId="tab-underline"
                      className="absolute inset-x-2 -bottom-px h-0.5 rounded-full bg-run shadow-[var(--shadow-glow-run)]"
                      transition={{ type: "spring", stiffness: 500, damping: 34 }}
                    />
                  )}
                </button>
              );
            })}
          </div>

          <div className="relative min-h-0 flex-1">
            <AnimatePresence mode="wait">
              <motion.div
                key={activeTab}
                className="absolute inset-0 flex flex-col [&>*]:min-h-0 [&>*]:flex-1"
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -4 }}
                transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
              >
                {activeTab === "overview" && <OverviewTab task={detail} events={events.data ?? []} />}
                {activeTab === "logs" && taskId && <LogsTab taskId={taskId} />}
                {activeTab === "diff" && taskId && <DiffTab taskId={taskId} revision={selectedRevision} />}
                {activeTab === "review" && taskId && <ReviewTab taskId={taskId} revision={selectedRevision} />}
                {activeTab === "governance" && <GovernanceTab task={detail} revision={selectedRevision} />}
              </motion.div>
            </AnimatePresence>
          </div>
        </div>
      </div>

      <ApprovalBar task={detail} />
    </div>
  );
}
