import { useMemo, useState } from "react";
import type { Review, ReviewDecision, ReviewIssue, Severity, TaskDetail, TaskEvent } from "@/generated/bindings";
import { agentLabel } from "@/copy/agents";
import { summarizeRisk, unresolvedIssues } from "@/lib/risk";
import { useDiff, useReview } from "@/hooks/useTaskData";
import { useGovernance } from "@/hooks/useGovernance";
import { useUiStore } from "@/stores/uiStore";
import { AgentMark } from "@/components/AgentMark";
import { IssueCard } from "@/components/IssueCard";
import { RiskSummaryCard } from "@/components/RiskSummaryCard";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { cn } from "@/lib/utils";
import { latestValidationOutcome } from "@/lib/execution/testReport";
import { AcceptanceCriteriaPanel } from "@/components/AcceptanceCriteriaPanel";

const DECISION: Record<ReviewDecision, { label: string; cls: string }> = {
  pass: { label: "通过", cls: "text-ok border-ok/50" },
  request_changes: { label: "要求修改", cls: "text-human border-human/50" },
  block: { label: "拦截", cls: "text-bad border-bad/50" },
};

const SEVERITY_ORDER: Severity[] = ["critical", "high", "medium", "low"];
const SEVERITY_LABEL: Record<Severity, string> = { critical: "严重", high: "高", medium: "中", low: "低" };

type IssueFilter = "all" | "blocking" | "unresolved";

function ReviewSummary({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false);
  const limit = 260;
  const long = text.length > limit;
  return (
    <div className="min-w-0">
      <p className="whitespace-pre-wrap text-[13px] leading-relaxed text-t2">
        {expanded || !long ? text : `${text.slice(0, limit).trimEnd()}…`}
      </p>
      {long && (
        <button type="button" onClick={() => setExpanded((v) => !v)} className="mt-1 text-[12px] text-run hover:underline">
          {expanded ? "收起完整点评" : "展开完整点评"}
        </button>
      )}
    </div>
  );
}

export function ReviewTab({ task, revision, events }: { task: TaskDetail; revision: number; events: TaskEvent[] }) {
  const taskId = task.id;
  const review = useReview(taskId, revision);
  const diff = useDiff(taskId, revision);
  const governance = useGovernance(taskId, revision);
  const requestDiffJump = useUiStore((s) => s.requestDiffJump);
  const setActiveTab = useUiStore((s) => s.setActiveTab);
  const [filter, setFilter] = useState<IssueFilter>("all");

  const risk = useMemo(() => summarizeRisk(diff.data?.files ?? []), [diff.data]);
  const unresolved = useMemo(() => unresolvedIssues(review.data), [review.data]);
  const validationOutcome = useMemo(() => latestValidationOutcome(events, revision), [events, revision]);

  const data: Review | null = review.data ?? null;
  const filtered = useMemo(() => {
    const issues = data?.issues ?? [];
    if (filter === "blocking") return issues.filter((i) => !i.resolved && (i.severity === "critical" || i.severity === "high"));
    if (filter === "unresolved") return issues.filter((i) => !i.resolved);
    return issues;
  }, [data, filter]);

  const grouped = useMemo(() => {
    const map = new Map<Severity, ReviewIssue[]>();
    for (const issue of filtered) {
      const list = map.get(issue.severity) ?? [];
      list.push(issue);
      map.set(issue.severity, list);
    }
    return map;
  }, [filtered]);

  if (revision < 1) return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4 overflow-y-auto px-6 py-5">
      <AcceptanceCriteriaPanel criteria={task.acceptanceCriteria} validation={validationOutcome} title="验收条件与审查证据" />
      <EmptyState title="这个 revision 还没有审查" />
    </div>
  );
  if (review.isLoading) return <div className="p-4"><SkeletonRows rows={5} /></div>;
  if (review.isError) return <ErrorState error={review.error} onRetry={() => review.refetch()} />;
  if (!data) return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4 overflow-y-auto px-6 py-5">
      <AcceptanceCriteriaPanel criteria={task.acceptanceCriteria} validation={validationOutcome} title="验收条件与审查证据" />
      <EmptyState title="这一轮还没有审查结果" hint="审查完成后会在这里列出结论与问题。" />
    </div>
  );

  const decision = DECISION[data.decision];
  const members = data.reviewerAgents ?? [];

  return (
    <div className="mx-auto max-w-3xl overflow-y-auto px-6 py-5">
      <div className="mb-4 flex items-baseline gap-3">
        <span className={`shrink-0 rounded-full border px-2.5 py-1 text-[12px] font-semibold ${decision.cls}`}>
          {decision.label}
        </span>
        {data.summary && <ReviewSummary text={data.summary} />}
      </div>

      <div className="mb-4">
        <AcceptanceCriteriaPanel
          criteria={task.acceptanceCriteria}
          validation={validationOutcome}
          review={data.decision}
          title="验收条件与审查证据"
        />
      </div>

      {members.length > 0 && (
        <div className="mb-4 flex flex-wrap items-center gap-2 text-[12px] text-t3">
          <span>审查成员 {members.length} 人</span>
          {members.map((agent) => (
            <span key={agent} className="flex items-center gap-1 rounded-full border border-line px-1.5 py-0.5">
              <AgentMark kind={agent} size={22} />
              <span className="text-t2">{agentLabel(agent)}</span>
            </span>
          ))}
        </div>
      )}

      {/* 风险不散落在普通 issue 列表里 (05 §6.9)。 */}
      <div className="mb-4">
        <RiskSummaryCard
          risk={risk}
          issues={unresolved}
          quality={governance.data?.quality}
          onFilterRisk={() => setActiveTab(taskId, "diff")}
        />
      </div>

      {data.issues.length === 0 ? (
        <EmptyState title="没有记录问题" hint="审查没有列出需要处理的问题。" />
      ) : (
        <>
          <div className="mb-3 flex flex-wrap gap-1" role="tablist" aria-label="问题筛选">
            <FilterTab id="all" active={filter} onClick={setFilter} label={`全部 ${data.issues.length}`} />
            <FilterTab id="unresolved" active={filter} onClick={setFilter} label={`未解决 ${unresolved.total}`} />
            <FilterTab
              id="blocking"
              active={filter}
              onClick={setFilter}
              label={`只看阻断项 ${unresolved.critical + unresolved.high}`}
              tone={unresolved.requiresExtraConfirmation ? "attention" : undefined}
            />
          </div>

          {filtered.length === 0 ? (
            <EmptyState title="当前筛选下没有问题" hint="切换到「全部」查看其余问题。" />
          ) : (
            <div className="flex flex-col gap-4">
              {SEVERITY_ORDER.filter((s) => grouped.has(s)).map((severity) => (
                <section key={severity}>
                  <div className="mb-2 flex items-center gap-2 text-[12px] font-semibold uppercase tracking-wider text-t3">
                    {SEVERITY_LABEL[severity]}
                    <span className="rounded-full bg-raised px-1.5 py-0.5 tabular-nums">{grouped.get(severity)!.length}</span>
                  </div>
                  {grouped.get(severity)!.map((issue) => (
                    <IssueCard
                      key={issue.id}
                      issue={issue}
                      memberCount={members.length}
                      onJump={(file, line) => requestDiffJump(taskId, file, line)}
                    />
                  ))}
                </section>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}

function FilterTab({
  id,
  active,
  onClick,
  label,
  tone,
}: {
  id: IssueFilter;
  active: IssueFilter;
  onClick: (id: IssueFilter) => void;
  label: string;
  tone?: "attention";
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active === id}
      onClick={() => onClick(id)}
      className={cn(
        "rounded-full border px-2.5 py-0.5 text-[12px] transition-colors",
        active === id ? "border-line-strong bg-raised text-t1" : "border-line text-t3 hover:text-t1",
        tone === "attention" && active !== id && "border-human/50 text-human"
      )}
    >
      {label}
    </button>
  );
}
