import { CircleAlert, CircleQuestionMark, TriangleAlert } from "lucide-react";
import type { BudgetUsage } from "@/generated/bindings";
import { budgetView, type BudgetLevel, type BudgetMetric } from "@/lib/governance/budget";
import { cn } from "@/lib/utils";

const LEVEL_NOTE: Record<BudgetLevel, string | null> = {
  ok: null,
  notice: "已用超过 70%，可以先确认剩余工作量。",
  warn: "已用超过 90%，接近上限。",
  blocked: "已达上限，任务会停在最近的安全检查点，不会中途破坏工作区。",
};

/**
 * 预算卡 (05 §6.2). 三件事必须同时成立：未知不显示成 0、已预留单独成段、
 * hard/soft/unavailable 用用户语言表达。
 */
export function BudgetPanel({ usage, onAdjust }: { usage: BudgetUsage; onAdjust?: () => void }) {
  const view = budgetView(usage);
  const note = LEVEL_NOTE[view.level];

  return (
    <section className="rounded-section border border-line bg-panel/60 p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          {view.level === "blocked" ? (
            <TriangleAlert className="size-4 text-bad" aria-hidden />
          ) : view.level === "warn" ? (
            <CircleAlert className="size-4 text-caution" aria-hidden />
          ) : null}
          <h2 className="font-semibold">预算</h2>
          <span
            className={cn(
              "text-meta",
              view.level === "blocked" ? "text-bad" : view.level === "warn" ? "text-caution" : "text-t2"
            )}
          >
            {view.headline}
          </span>
        </div>
        {onAdjust && view.level !== "ok" && (
          <button type="button" onClick={onAdjust} className="text-meta text-link hover:underline">
            调整预算上限
          </button>
        )}
      </div>

      {note && <p className="mt-1.5 text-meta text-t3">{note}</p>}

      <div className="mt-3 grid gap-3 sm:grid-cols-3">
        {view.metrics.map((metric) => (
          <MetricCard key={metric.key} metric={metric} />
        ))}
      </div>

      {view.exceeded && (
        <p className="mt-3 rounded-control border border-caution/50 bg-caution-bg px-3 py-2 text-meta text-t1">
          提高上限后，任务会从最近保存的检查点继续，已经完成的开发和验证不会重跑。
        </p>
      )}
    </section>
  );
}

function MetricCard({ metric }: { metric: BudgetMetric }) {
  const unknown = !metric.known;
  return (
    <div className="rounded-section border border-line bg-app/40 p-3">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-meta font-medium">{metric.label}</span>
        <span className={cn("text-meta", unknown ? "text-t3" : "text-t2")}>
          {metric.display} / {metric.limitDisplay}
        </span>
      </div>

      <div className="mt-2 flex h-1.5 overflow-hidden rounded-pill bg-line" role="img" aria-label={`${metric.label}用量`}>
        {unknown ? (
          // 用量未知就不画进度——画一段 0 会被读成「几乎没用」(05 §6.2)。
          <span className="h-full w-full bg-[repeating-linear-gradient(45deg,var(--color-line-strong)_0_4px,transparent_4px_8px)]" />
        ) : (
          <>
            <span
              className={cn(
                "h-full",
                metric.level === "blocked" ? "bg-bad" : metric.level === "warn" ? "bg-caution" : "bg-selection"
              )}
              style={{ width: `${metric.settledPercent}%` }}
            />
            {/* 已预留但尚未结算，与已结算区分开。 */}
            <span
              className="h-full bg-selection/35 bg-[repeating-linear-gradient(45deg,transparent_0_3px,rgba(0,0,0,0.08)_3px_6px)]"
              style={{ width: `${metric.reservedPercent}%` }}
            />
          </>
        )}
      </div>

      <div className="mt-1.5 flex items-center gap-1 text-meta text-t3">
        {unknown && <CircleQuestionMark className="size-3 shrink-0" aria-hidden />}
        <span>{metric.enforcementLabel}</span>
      </div>
      {metric.note && <p className="mt-1 text-meta leading-relaxed text-t3">{metric.note}</p>}
    </div>
  );
}
