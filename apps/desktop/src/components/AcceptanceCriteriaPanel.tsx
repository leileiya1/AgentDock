import { AlertTriangle, CheckCircle2, CircleDashed, UserCheck, XCircle } from "lucide-react";
import type { AcceptanceCriterion, AcceptanceCriterionKind, ReviewDecision } from "@/generated/bindings";
import type { ValidationOutcome } from "@/lib/execution/testReport";
import { acceptanceStatus, type AcceptanceStatus } from "@/lib/acceptance";

const KIND_LABEL: Record<AcceptanceCriterionKind, string> = {
  build: "构建",
  test: "测试",
  behavior: "行为",
  manual: "人工",
};

const STATUS_META: Record<AcceptanceStatus, { label: string; className: string; icon: typeof CheckCircle2 }> = {
  passed: { label: "通过", className: "text-ok", icon: CheckCircle2 },
  failed: { label: "未通过", className: "text-bad", icon: XCircle },
  pending: { label: "待验证", className: "text-t3", icon: CircleDashed },
  unverified: { label: "未运行验证", className: "text-caution", icon: AlertTriangle },
  manual: { label: "待人工确认", className: "text-status-human", icon: UserCheck },
};

interface Props {
  criteria: AcceptanceCriterion[];
  validation: ValidationOutcome;
  review?: ReviewDecision | null;
  title?: string;
  manualChecked?: ReadonlySet<string>;
  onManualChange?: (id: string, checked: boolean) => void;
}

export function AcceptanceCriteriaPanel({
  criteria,
  validation,
  review = null,
  title = "验收条件",
  manualChecked,
  onManualChange,
}: Props) {
  if (criteria.length === 0) return null;
  const entries = criteria.map((criterion) => {
    const confirmed = manualChecked?.has(criterion.id) ?? false;
    return { criterion, confirmed, status: acceptanceStatus(criterion.kind, validation, review, confirmed) };
  });
  const passed = entries.filter((entry) => entry.status === "passed").length;
  const needsAttention = entries.filter((entry) => entry.status === "failed" || entry.status === "unverified").length;
  const waiting = entries.length - passed - needsAttention;
  return (
    <section className="rounded-section border border-line bg-app/60 p-3" aria-label={title}>
      <div className="mb-2 flex items-center justify-between gap-3">
        <h3 className="text-body font-semibold text-t1">{title}</h3>
        <span className="flex flex-wrap justify-end gap-x-2 text-meta">
          <span className="text-ok">通过 {passed}</span>
          {needsAttention > 0 && <span className="text-status-human">需处理 {needsAttention}</span>}
          {waiting > 0 && <span className="text-t3">待确认 {waiting}</span>}
          <span className="text-t3">共 {criteria.length} 条</span>
        </span>
      </div>
      <ol className="flex flex-col gap-2">
        {entries.map(({ criterion, confirmed, status }) => {
          const meta = STATUS_META[status];
          const Icon = meta.icon;
          const row = (
            <>
              <span className="shrink-0 rounded-pill border border-line px-1.5 py-0.5 text-micro text-t3">{KIND_LABEL[criterion.kind]}</span>
              <span className="min-w-0 flex-1 text-body text-t1">{criterion.text}</span>
              <span className={`flex shrink-0 items-center gap-1 text-meta font-medium ${meta.className}`}>
                <Icon className="size-3.5" aria-hidden /> {meta.label}
              </span>
            </>
          );
          return criterion.kind === "manual" && onManualChange ? (
            <li key={criterion.id}>
              <label className="flex cursor-pointer items-start gap-2 rounded-control border border-line/70 px-2.5 py-2 hover:bg-raised/50">
                <input
                  type="checkbox"
                  checked={confirmed}
                  onChange={(event) => onManualChange(criterion.id, event.target.checked)}
                  className="mt-1 accent-[var(--color-status-human)]"
                />
                <span className="flex min-w-0 flex-1 items-center gap-2">{row}</span>
              </label>
            </li>
          ) : (
            <li key={criterion.id} className="flex items-center gap-2 rounded-control border border-line/70 px-2.5 py-2">{row}</li>
          );
        })}
      </ol>
    </section>
  );
}
