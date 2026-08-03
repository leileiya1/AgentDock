import { useMemo, useState } from "react";
import { ListTree } from "lucide-react";
import type { RunSummary } from "@/generated/bindings";
import { useLayout } from "@/hooks/useBreakpoint";
import type { ExecutionTree } from "@/lib/execution/tree";
import type { ResultCard } from "@/lib/logs/resultCard";
import { useReview } from "@/hooks/useTaskData";
import { useUiStore } from "@/stores/uiStore";
import { RunLogViewer } from "@/components/RunLogViewer";
import { RunList, countRuns } from "@/components/logs/RunList";
import { EmptyState } from "@/components/EmptyState";

interface Props {
  taskId: string;
  tree: ExecutionTree;
  runs: RunSummary[];
  revision: number;
  lastActivityAt: string | null;
}

/**
 * 日志页 (05 §7): 执行树、run 列表、内容三者联动。左侧执行树负责在轮次和阶段间导航，
 * 这里只负责「选择某阶段的具体执行者」和展示结果。
 */
export function LogsTab({ taskId, tree, runs, revision, lastActivityAt }: Props) {
  const selectedRunId = useUiStore((s) => s.selectedRun[taskId]);
  const selectTreeNode = useUiStore((s) => s.selectTreeNode);
  const layout = useLayout();
  const [listOpen, setListOpen] = useState(false);

  const revisionRuns = useMemo(
    () => runs.filter((run) => run.revision === revision),
    [runs, revision]
  );

  // 选中的 run 必须属于当前 revision，否则回退到该轮最新的一次运行。
  const activeRun = useMemo(() => {
    const fromSelection = revisionRuns.find((run) => run.id === selectedRunId);
    if (fromSelection) return fromSelection;
    return revisionRuns
      .slice()
      .sort((a, b) => Date.parse(b.startedAt ?? "") - Date.parse(a.startedAt ?? ""))[0];
  }, [revisionRuns, selectedRunId]);

  // 审查结论已经是结构化数据，比从日志里猜更可靠 (05 §4.4)。
  const review = useReview(
    taskId,
    activeRun?.role === "reviewer" && activeRun.status === "SUCCEEDED" ? activeRun.revision : undefined
  );
  const reviewCard: ResultCard | null = useMemo(() => {
    if (!review.data) return null;
    const unresolved = review.data.issues.filter((issue) => !issue.resolved);
    return {
      conclusion:
        review.data.summary ??
        (review.data.decision === "pass" ? "审查通过。" : "审查要求修改。"),
      completed: [],
      validation: [],
      concerns: unresolved.map((issue) => issue.title),
      nextAction: unresolved.length > 0 ? `需要处理 ${unresolved.length} 个未解决问题` : null,
      structured: true,
    };
  }, [review.data]);

  if (revisionRuns.length === 0) {
    return (
      <EmptyState
        title={`r${revision} 还没有运行`}
        hint="任务开始后，这里会按阶段列出每一次开发、验证与审查。"
      />
    );
  }

  // 只有一个 run 时收起中间列表，把空间让给内容 (05 §5.1)；
  // 1440px 以下三栏放不下，列表默认折叠成一个按钮 (05 §8)。
  const hasMultipleRuns = countRuns(tree, revision) > 1;
  const showList = hasMultipleRuns && (layout === "wide" || listOpen);

  return (
    <div className="flex h-full min-h-0">
      {showList && (
        <div className="w-64 shrink-0 overflow-y-auto border-r border-line/70">
          <div className="flex items-center justify-between px-2 pt-2">
            <span className="text-meta text-t3">执行者</span>
            {layout !== "wide" && (
              <button
                type="button"
                onClick={() => setListOpen(false)}
                className="rounded-control px-1.5 py-0.5 text-meta text-t3 hover:bg-raised hover:text-t1"
              >
                收起
              </button>
            )}
          </div>
          <RunList
            tree={tree}
            revision={revision}
            selectedRunId={activeRun?.id ?? null}
            onSelect={(row) => {
              selectTreeNode(taskId, { revision, phase: row.phase, runId: row.runId });
              if (layout === "compact") setListOpen(false);
            }}
          />
        </div>
      )}
      {hasMultipleRuns && !showList && (
        <button
          type="button"
          onClick={() => setListOpen(true)}
          aria-label="展开执行者列表"
          className="flex shrink-0 items-center gap-1 border-r border-line/70 px-1.5 text-meta text-t3 transition-colors [writing-mode:vertical-rl] hover:bg-raised hover:text-t1"
        >
          <ListTree className="size-3.5 rotate-90" aria-hidden />
          执行者 {countRuns(tree, revision)}
        </button>
      )}
      <div className="flex min-w-0 flex-1 flex-col">
        {activeRun ? (
          <RunLogViewer
            key={activeRun.id}
            run={activeRun}
            preferredCard={reviewCard}
            lastActivityAt={lastActivityAt}
          />
        ) : (
          <EmptyState title="选择一个运行查看结果" />
        )}
      </div>
    </div>
  );
}
