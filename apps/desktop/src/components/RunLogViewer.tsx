import { useMemo } from "react";
import { Settings2 } from "lucide-react";
import type { RunSummary } from "@/generated/bindings";
import { useRunLog } from "@/hooks/useRunLog";
import { buildResultCard, liveProgress, type ResultCard as ResultCardModel } from "@/lib/logs/resultCard";
import { useUiStore } from "@/stores/uiStore";
import { cn } from "@/lib/utils";
import { ErrorState } from "./ErrorState";
import { ResultCard } from "./logs/ResultCard";
import { LiveProgress } from "./logs/LiveProgress";
import { TechnicalLog } from "./logs/TechnicalLog";

interface Props {
  run: RunSummary;
  /** 审查结论等已经结构化的内容，优先于日志推导 (05 §4.4)。 */
  preferredCard?: ResultCardModel | null;
  lastActivityAt?: string | null;
}

const ROLE_TITLE: Record<RunSummary["role"], string> = {
  planner: "计划结果",
  developer: "开发结果",
  reviewer: "审查结果",
  validator: "验证结果",
};

/**
 * 右侧内容区 (05 §4.1): 顶部「结果优先」——本次结果 / 主要内容 / 实时进展；
 * 技术详情（逐条智能体事件 + 图标）默认折叠，避免原始协议掩盖可读结论；用户可按需展开。
 */
export function RunLogViewer({ run, preferredCard, lastActivityAt }: Props) {
  const { buffer, loading, error, loadMore, hasMore } = useRunLog(run.id);
  const technicalOpen = useUiStore((s) => s.technicalLogOpen);
  const setTechnicalOpen = useUiStore((s) => s.setTechnicalLogOpen);

  const lines = useMemo(() => buffer?.lines ?? [], [buffer]);
  const running = run.status === "RUNNING";

  const card = useMemo(
    () => preferredCard ?? (running ? null : buildResultCard(lines)),
    [preferredCard, running, lines]
  );
  const progress = useMemo(() => (running ? liveProgress(lines) : []), [running, lines]);

  const elapsedSecs = run.startedAt
    ? Math.max(0, Math.round((Date.now() - Date.parse(run.startedAt)) / 1000))
    : null;
  const stalled = !!lastActivityAt && Date.now() - Date.parse(lastActivityAt) > 10_000;

  if (error && lines.length === 0) return <ErrorState error={error} onRetry={loadMore} />;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center justify-end border-b border-line/70 px-3 py-1.5">
        <button
          type="button"
          onClick={() => setTechnicalOpen(!technicalOpen)}
          aria-expanded={technicalOpen}
          className={cn(
            "flex items-center gap-1 rounded-md px-2 py-1 text-[12px] transition-colors",
            technicalOpen ? "bg-raised text-t2" : "text-t3 hover:bg-raised hover:text-t1"
          )}
        >
          <Settings2 className="size-3.5" /> {technicalOpen ? "收起技术详情" : "技术详情"}
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4">
        <div className="mx-auto flex max-w-3xl flex-col gap-4">
          {running ? (
            <LiveProgress
              events={progress}
              elapsedSecs={elapsedSecs}
              lastActivityAt={lastActivityAt ?? null}
              stalled={stalled}
            />
          ) : (
            <ResultCard card={card} title={ROLE_TITLE[run.role]} onOpenTechnical={() => setTechnicalOpen(true)} />
          )}
        </div>
      </div>

      {technicalOpen && (
        <div className="flex h-1/2 min-h-0 shrink-0 flex-col border-t border-line">
          <TechnicalLog
            runId={run.id}
            lines={lines}
            headTrimmed={!!buffer?.headTrimmed}
            loading={loading}
            hasMore={hasMore}
            onLoadMore={loadMore}
          />
        </div>
      )}
    </div>
  );
}
