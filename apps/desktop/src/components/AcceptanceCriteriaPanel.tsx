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
  unverified: { label: "未运行验证", className: "text-human", icon: AlertTriangle },
  manual: { label: "待人工确认", className: "text-human", icon: UserCheck },
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
  return (
    <section className="rounded-[var(--radius-panel)] border border-line bg-app/60 p-3" aria-label={title}>
      <div className="mb-2 flex items-center justify-between gap-3">
        <h3 className="text-[13px] font-semibold text-t1">{title}</h3>
        <span className="text-[11px] text-t3">{criteria.length} 条 · 按真实证据判定</span>
      </div>
      <ol className="flex flex-col gap-2">
        {criteria.map((criterion) => {
          const confirmed = manualChecked?.has(criterion.id) ?? false;
          const status = acceptanceStatus(criterion.kind, validation, review, confirmed);
          const meta = STATUS_META[status];
          const Icon = meta.icon;
          const row = (
            <>
              <span className="shrink-0 rounded-full border border-line px-1.5 py-0.5 text-[10px] text-t3">{KIND_LABEL[criterion.kind]}</span>
              <span className="min-w-0 flex-1 text-[13px] text-t1">{criterion.text}</span>
              <span className={`flex shrink-0 items-center gap-1 text-[11px] font-medium ${meta.className}`}>
                <Icon className="size-3.5" aria-hidden /> {meta.label}
              </span>
            </>
          );
          return criterion.kind === "manual" && onManualChange ? (
            <li key={criterion.id}>
              <label className="flex cursor-pointer items-start gap-2 rounded-md border border-line/70 px-2.5 py-2 hover:bg-raised/50">
                <input
                  type="checkbox"
                  checked={confirmed}
                  onChange={(event) => onManualChange(criterion.id, event.target.checked)}
                  className="mt-1 accent-[var(--color-human)]"
                />
                <span className="flex min-w-0 flex-1 items-center gap-2">{row}</span>
              </label>
            </li>
          ) : (
            <li key={criterion.id} className="flex items-center gap-2 rounded-md border border-line/70 px-2.5 py-2">{row}</li>
          );
        })}
      </ol>
    </section>
  );
}
