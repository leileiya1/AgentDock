import { ArrowRight, FileWarning, Minus, Plus } from "lucide-react";
import type { PlanReviewContext } from "@/generated/bindings";

export function PlanVersionComparison({ context }: { context: PlanReviewContext }) {
  const diff = context.latestDiff;
  const previous = context.plans.at(-2);
  const latest = context.plans.at(-1);
  return (
    <div className="space-y-3">
      {context.detectedDeviations.length > 0 && (
        <div className="rounded-control border border-caution/40 bg-caution-bg p-3 text-meta">
          <div className="flex items-center gap-1.5 font-medium text-caution"><FileWarning className="size-4" /> 上轮检测到越界文件</div>
          <div className="mt-2 flex flex-wrap gap-1.5">
            {context.detectedDeviations.map((path) => <code key={path} className="rounded bg-app/70 px-2 py-1 text-meta text-t1">{path}</code>)}
          </div>
          <p className="mt-2 text-meta text-t2">上轮工作区已重置；批准前确认新计划是否确实覆盖这些文件。</p>
        </div>
      )}
      {diff && previous && latest ? (
        <div className="rounded-control border border-line bg-raised p-3 text-meta">
          <div className="flex items-center gap-2 font-medium text-t1">
            计划 v{diff.fromVersion} <ArrowRight className="size-3.5 text-t3" /> v{diff.toVersion}
          </div>
          {previous.rejectionReason && <p className="mt-2 rounded bg-app/60 p-2 text-t2">上次驳回：{previous.rejectionReason}</p>}
          <div className="mt-3 grid grid-cols-2 gap-2">
            <PlanSummary label={`v${previous.plan.version}`} value={previous.plan.summary} />
            <PlanSummary label={`v${latest.plan.version}`} value={latest.plan.summary} />
          </div>
          <div className="mt-3 grid gap-2">
            <ChangeRow label="步骤" added={diff.addedSteps} removed={diff.removedSteps} />
            <ChangeRow label="允许路径" added={diff.addedAllowedPaths} removed={diff.removedAllowedPaths} mono />
            <ChangeRow label="风险" added={diff.addedRisks} removed={diff.removedRisks} />
          </div>
          {!diff.summaryChanged && !hasListChanges(diff) && <p className="mt-2 text-t3">新版计划与上一版没有结构化差异。</p>}
        </div>
      ) : (
        <div className="rounded-control border border-line bg-raised p-3 text-meta text-t3">这是首个计划版本，没有上一版可比较。</div>
      )}
    </div>
  );
}

function PlanSummary({ label, value }: { label: string; value: string }) {
  return <div className="rounded border border-line bg-app/50 p-2"><div className="text-micro font-medium text-t3">{label} 摘要</div><p className="mt-1 line-clamp-4 leading-relaxed text-t2">{value}</p></div>;
}

function ChangeRow({ label, added, removed, mono = false }: { label: string; added: string[]; removed: string[]; mono?: boolean }) {
  if (!added.length && !removed.length) return null;
  return (
    <div className="grid grid-cols-[72px_1fr] gap-2">
      <span className="text-t3">{label}</span>
      <div className={`space-y-1 ${mono ? "font-mono text-meta" : ""}`}>
        {added.map((value) => <div key={`add-${value}`} className="flex items-start gap-1 text-ok"><Plus className="mt-0.5 size-3 shrink-0" />{value}</div>)}
        {removed.map((value) => <div key={`remove-${value}`} className="flex items-start gap-1 text-caution"><Minus className="mt-0.5 size-3 shrink-0" />{value}</div>)}
      </div>
    </div>
  );
}

function hasListChanges(diff: NonNullable<PlanReviewContext["latestDiff"]>) {
  return [diff.addedSteps, diff.removedSteps, diff.addedAllowedPaths, diff.removedAllowedPaths, diff.addedRisks, diff.removedRisks].some((values) => values.length > 0);
}
