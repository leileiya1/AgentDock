import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { motion } from "motion/react";
import { PanelLeft } from "lucide-react";
import type { DetailTab } from "@/stores/uiStore";
import { useTaskDetail } from "@/hooks/useTasks";
import { useUiStore } from "@/stores/uiStore";
import { useRunLogStream } from "@/hooks/useRunLogStream";
import { useExecutionTree } from "@/hooks/useExecutionTree";
import { useLayout } from "@/hooks/useBreakpoint";
import { cn } from "@/lib/utils";
import { ApprovalBar } from "@/components/ApprovalBar";
import { PermissionGate } from "@/components/permission/PermissionGate";
import { SidePanel } from "@/components/SidePanel";
import { TaskHeader } from "@/components/TaskHeader";
import { ExecutionTree } from "@/components/execution/ExecutionTree";
import { LiveStatusBar } from "@/components/execution/LiveStatusBar";
import { StopRunButton } from "@/components/execution/StopRunButton";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { OverviewTab } from "@/routes/detail/OverviewTab";
import { LogsTab } from "@/routes/detail/LogsTab";
import { DiffTab } from "@/routes/detail/DiffTab";
import { ReviewTab } from "@/routes/detail/ReviewTab";
import { GovernanceTab } from "@/routes/detail/GovernanceTab";
import { isAgentRunning, isTaskExecuting } from "@/lib/taskStatus";

const TABS: Array<{ id: DetailTab; label: string; key: string }> = [
  { id: "overview", label: "概览", key: "1" },
  { id: "logs", label: "日志", key: "2" },
  { id: "diff", label: "Diff", key: "3" },
  { id: "review", label: "审查", key: "4" },
  { id: "governance", label: "治理", key: "5" },
];

// Only these phases run a supervised Agent process with a registered cancellation token, so only
// here is "停止" both accurate (there really is an Agent) and safe. VALIDATING (build/test) and
// MERGING (git) hold no agent_run, so cancelling there would force-remove the worktree out from
// under a live build — exclude them from the stop affordance.

export function TaskDetail() {
  const { taskId, projectId } = useParams();
  const task = useTaskDetail(taskId);

  const activeTab = useUiStore((s) => (taskId ? s.activeTab[taskId] : undefined)) ?? "overview";
  const setActiveTab = useUiStore((s) => s.setActiveTab);
  const selectedRevStore = useUiStore((s) => (taskId ? s.selectedRevision[taskId] : undefined));
  const treeSelection = useUiStore((s) => (taskId ? s.treeSelection[taskId] : undefined)) ?? null;
  const selectTreeNode = useUiStore((s) => s.selectTreeNode);

  useRunLogStream();

  const layout = useLayout();
  const [treeDrawerOpen, setTreeDrawerOpen] = useState(false);

  const detail = task.data;
  const execution = useExecutionTree(detail);
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

  // Events drive updates; this is only the recovery poll for a dropped subscription.
  useEffect(() => {
    if (!detail || !isTaskExecuting(detail.status)) return;
    const timer = setTimeout(() => {
      task.refetch();
      execution.refetch();
    }, 60_000);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detail?.updatedAt, detail?.status]);

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
    <div className="flex h-full min-h-0 w-full flex-col overflow-hidden">
      <TaskHeader
        task={detail}
        projectId={projectId}
        phaseSummary={execution.live ? [execution.live.headline, execution.live.detail].filter(Boolean).join(" · ") : null}
      />

      <div className="flex min-h-0 flex-1">
        {/* 执行树需要容纳 28–30 px Provider 图标 + 名称 + 状态词 (05 §5.4)；
            窄屏放不下就收进抽屉，而不是把正文挤没 (05 §8)。 */}
        <SidePanel
          drawer={layout === "compact"}
          open={treeDrawerOpen}
          onClose={() => setTreeDrawerOpen(false)}
          title="执行树"
          width="19rem"
        >
          <div className="px-2 py-2">
            {execution.isLoading || !execution.tree ? (
              <SkeletonRows rows={5} gap={14} />
            ) : execution.error ? (
              <ErrorState error={execution.error} onRetry={() => execution.refetch()} compact />
            ) : (
              <ExecutionTree
                tree={execution.tree}
                selection={treeSelection}
                onSelect={(selection) => {
                  if (taskId) {
                    selectTreeNode(taskId, selection);
                    if (selection.runId) setActiveTab(taskId, "logs");
                  }
                  // 抽屉里选完就收起，否则内容被自己挡住。
                  if (layout === "compact") setTreeDrawerOpen(false);
                }}
                revisionStats={detail.revisions}
              />
            )}
          </div>
        </SidePanel>

        <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
          <div className="flex shrink-0 items-start gap-2 px-4 pt-3">
            {layout === "compact" && (
              <button
                type="button"
                onClick={() => setTreeDrawerOpen(true)}
                aria-label="打开执行树"
                className="mt-0.5 flex shrink-0 items-center gap-1 rounded-md border border-line px-2 py-1.5 text-[12px] text-t2 transition-colors hover:bg-raised hover:text-t1"
              >
                <PanelLeft className="size-3.5" aria-hidden /> 执行树
              </button>
            )}
            <div className="min-w-0 flex-1">
              <LiveStatusBar status={execution.live} />
            </div>
            {/* 只有真正跑着 Agent 的阶段才给「停止」入口 (§14/§17)：这些阶段可安全中止，
                构建/合并阶段没有 Agent 进程，停止会与工作区清理竞争，故不显示。 */}
            {isAgentRunning(detail.status) && <StopRunButton taskId={detail.id} />}
          </div>
          <div
            className="flex shrink-0 gap-1 overflow-x-auto border-b border-line/70 px-4 pt-2"
            role="tablist"
            aria-label="任务详情"
          >
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
            {/* Tab content switches instantly and robustly. We intentionally avoid AnimatePresence +
                `mode="wait"` here: waiting for an exit animation to finish before mounting the next
                tab means a stalled exit (throttled rAF in a backgrounded window, extreme jank) can
                leave the user unable to switch tabs, and a lingering exiting pane can bleed through
                the incoming one. `initial={false}` renders each tab at its resting state immediately,
                so the content is always correct even if animations never tick. The active-tab
                underline still animates via its shared `layoutId`. */}
            <div className="absolute inset-0">
              <motion.div
                key={activeTab}
                className="absolute inset-0 flex flex-col [&>*]:min-h-0 [&>*]:flex-1"
                initial={false}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
              >
                {activeTab === "overview" && <OverviewTab task={detail} events={execution.events} />}
                {activeTab === "logs" && taskId && execution.tree && (
                  <LogsTab
                    taskId={taskId}
                    tree={execution.tree}
                    runs={execution.runs}
                    revision={selectedRevision}
                    lastActivityAt={execution.lastActivityAt}
                  />
                )}
                {activeTab === "diff" && taskId && <DiffTab taskId={taskId} revision={selectedRevision} />}
                {activeTab === "review" && taskId && (
                  <ReviewTab task={detail} revision={selectedRevision} events={execution.events} />
                )}
                {activeTab === "governance" && <GovernanceTab task={detail} revision={selectedRevision} />}
              </motion.div>
            </div>
          </div>
        </div>
      </div>

      <PermissionGate taskId={detail.id} projectId={projectId} taskStatus={detail.status} />
      <ApprovalBar task={detail} />
    </div>
  );
}
