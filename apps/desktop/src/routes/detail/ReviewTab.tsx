import { useMemo, useState } from "react";
import type { Review, ReviewDecision, ReviewIssue, Severity } from "@/generated/bindings";
import { useReview } from "@/hooks/useTaskData";
import { useUiStore } from "@/stores/uiStore";
import { IssueCard } from "@/components/IssueCard";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { Badge } from "@/components/ui/badge";

const DECISION: Record<ReviewDecision, { label: string; cls: string }> = {
  pass: { label: "通过", cls: "text-ok border-ok/50" },
  request_changes: { label: "要求返工", cls: "text-bad border-bad/50" },
  block: { label: "拦截", cls: "text-bad border-bad/50" },
};

const SEVERITY_ORDER: Severity[] = ["critical", "high", "medium", "low"];
const SEVERITY_GROUP_LABEL: Record<Severity, string> = { critical: "严重", high: "高", medium: "中", low: "低" };

function ReviewSummary({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false);
  const limit = 260;
  const long = text.length > limit;
  const shown = expanded || !long ? text : `${text.slice(0, limit).trimEnd()}…`;
  return (
    <div className="min-w-0">
      <p className="whitespace-pre-wrap text-[13px] leading-relaxed text-t2">{shown}</p>
      {long && (
        <button type="button" onClick={() => setExpanded((value) => !value)} className="mt-1 text-[12px] text-run hover:underline">
          {expanded ? "收起完整点评" : "展开完整点评"}
        </button>
      )}
    </div>
  );
}

export function ReviewTab({ taskId, revision }: { taskId: string; revision: number }) {
  const review = useReview(taskId, revision);
  const requestDiffJump = useUiStore((s) => s.requestDiffJump);

  const grouped = useMemo(() => {
    const g = new Map<Severity, ReviewIssue[]>();
    for (const issue of review.data?.issues ?? []) {
      const list = g.get(issue.severity) ?? [];
      list.push(issue);
      g.set(issue.severity, list);
    }
    return g;
  }, [review.data]);

  if (revision < 1) return <EmptyState title="这个 revision 还没有审查" />;
  if (review.isLoading) return <div className="p-4"><SkeletonRows rows={5} /></div>;
  if (review.isError) return <ErrorState error={review.error} onRetry={() => review.refetch()} />;

  const data: Review | null = review.data ?? null;
  if (!data) return <EmptyState title="这一轮还没有审查结果" hint="审查完成后会在这里列出结论与问题。" />;

  const decision = DECISION[data.decision];

  return (
    <div className="mx-auto max-w-3xl overflow-y-auto px-6 py-5">
      <div className="mb-4 flex items-baseline gap-3">
        <span className={`shrink-0 rounded-full border px-2.5 py-1 text-[12px] font-semibold ${decision.cls}`}>
          {decision.label}
        </span>
        {data.summary && <ReviewSummary text={data.summary} />}
      </div>
      {(data.reviewerAgents?.length ?? 0) > 0 && (
        <div className="mb-4 flex flex-wrap items-center gap-1.5 text-[12px] text-t3">
          <span>审查成员</span>
          {data.reviewerAgents?.map((agent) => <Badge key={agent}>{agent}</Badge>)}
        </div>
      )}

      {data.issues.length === 0 ? (
        <EmptyState title="没有记录问题" hint="审查没有列出需要处理的问题。" />
      ) : (
        <div className="flex flex-col gap-4">
          {SEVERITY_ORDER.filter((s) => grouped.has(s)).map((sev) => (
            <section key={sev}>
              <div className="mb-2 flex items-center gap-2 text-[12px] font-semibold uppercase tracking-wider text-t3">
                {SEVERITY_GROUP_LABEL[sev]}
                <Badge>{grouped.get(sev)!.length}</Badge>
              </div>
              {grouped.get(sev)!.map((issue) => (
                <IssueCard key={issue.id} issue={issue} onJump={(file, line) => requestDiffJump(taskId, file, line)} />
              ))}
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
