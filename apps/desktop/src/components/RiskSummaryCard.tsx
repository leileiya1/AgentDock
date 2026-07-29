import { ShieldCheck, TriangleAlert } from "lucide-react";
import type { QualityEvaluation } from "@/generated/bindings";
import { RISK_META, type RiskSummary, type UnresolvedIssues } from "@/lib/risk";
import { cn } from "@/lib/utils";

/**
 * 「安全与完整性」汇总卡 (05 §6.9). 风险不散落在普通 issue 列表里；
 * 质量门禁失败要说清是哪一条确定性规则，而不是归因给「AI 觉得不通过」。
 */
export function RiskSummaryCard({
  risk,
  issues,
  quality,
  onFilterRisk,
}: {
  risk: RiskSummary;
  issues: UnresolvedIssues;
  quality?: QualityEvaluation | null;
  onFilterRisk?: () => void;
}) {
  const failedChecks = (quality?.checks ?? []).filter((check) => !check.passed);
  const clean = !risk.hasHighRisk && risk.byFile.size === 0 && issues.total === 0 && failedChecks.length === 0;

  return (
    <section
      className={cn(
        "rounded-[var(--radius-panel)] border p-4",
        clean ? "border-line bg-panel/60" : "border-human/50 bg-human-bg"
      )}
    >
      <div className="flex items-center gap-2">
        {clean ? (
          <ShieldCheck className="size-4 shrink-0 text-ok" aria-hidden />
        ) : (
          <TriangleAlert className="size-4 shrink-0 text-human" aria-hidden />
        )}
        <h2 className="text-[13px] font-semibold">安全与完整性</h2>
        <span className="text-[12px] text-t2">
          {clean ? "没有发现需要额外确认的风险" : "以下内容需要你确认后才适合批准"}
        </span>
      </div>

      {!clean && (
        <ul className="mt-3 list-none space-y-2 text-[13px]">
          {issues.critical + issues.high > 0 && (
            <Row label="未解决的高风险问题">
              {issues.critical > 0 && `${issues.critical} 个严重`}
              {issues.critical > 0 && issues.high > 0 && "、"}
              {issues.high > 0 && `${issues.high} 个高风险`}
            </Row>
          )}

          {(Object.keys(risk.counts) as Array<keyof typeof risk.counts>)
            .filter((kind) => risk.counts[kind] > 0)
            .map((kind) => (
              <Row key={kind} label={RISK_META[kind].label}>
                {risk.counts[kind]} 个文件 · {RISK_META[kind].why}
              </Row>
            ))}

          {risk.removedTests.length > 0 && (
            <Row label="测试变化">
              删除 {risk.removedTests.length} 个、改动 {risk.touchedTests.length} 个测试文件：
              {risk.removedTests.slice(0, 3).join("、")}
            </Row>
          )}

          {failedChecks.map((check) => (
            <Row key={check.name} label="质量门禁">
              {/* 门禁是确定性规则，说明具体是哪一条。 */}
              未通过「{qualityLabel(check.name)}」（{check.weight} 分）：{check.detail}
            </Row>
          ))}
        </ul>
      )}

      {!clean && onFilterRisk && risk.byFile.size > 0 && (
        <button type="button" onClick={onFilterRisk} className="mt-3 text-[12px] text-run hover:underline">
          在 Diff 中只看风险文件
        </button>
      )}
    </section>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <li className="flex gap-2">
      <span className="w-24 shrink-0 text-[12px] text-t3">{label}</span>
      <span className="min-w-0 flex-1 text-t1">{children}</span>
    </li>
  );
}

const QUALITY_LABEL: Record<string, string> = {
  validation: "自动验证",
  independent_review: "独立审查",
  high_risk_issues: "高风险问题",
  control_plane_changes: "控制面变更",
};

export const qualityLabel = (name: string) => QUALITY_LABEL[name] ?? name;
