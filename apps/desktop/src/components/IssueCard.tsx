import { useState } from "react";
import { ChevronRight } from "lucide-react";
import type { ReviewIssue, Severity } from "@/generated/bindings";
import { agentLabel } from "@/copy/agents";
import { AgentMark } from "@/components/AgentMark";
import { cn } from "@/lib/utils";

const SEVERITY_LABEL: Record<Severity, string> = { critical: "严重", high: "高", medium: "中", low: "低" };

function severityColor(sev: Severity): string {
  return sev === "critical" || sev === "high" ? "var(--color-bad)" : "var(--color-t3)";
}

interface Props {
  issue: ReviewIssue;
  /** 委员会成员总数，用于把「几人同意」表达成 2/3 而不是裸数字 (05 §6.11)。 */
  memberCount?: number;
  onJump?: (file: string, line: number | null) => void;
}

export function IssueCard({ issue, memberCount, onJump }: Props) {
  const [open, setOpen] = useState(false);
  const reporters = issue.reportedBy ?? [];
  const color = severityColor(issue.severity);
  const isHigh = issue.severity === "critical" || issue.severity === "high";
  const loc = issue.file != null ? `${issue.file}${issue.lineStart != null ? `:${issue.lineStart}` : ""}` : null;

  return (
    <div className={cn("mb-2 flex gap-2 rounded-[var(--radius-panel)] border border-line bg-panel p-3", issue.resolved && "opacity-50")}>
      <span className="w-[3px] shrink-0 rounded-full" style={{ background: color }} aria-hidden />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className={cn("shrink-0 text-[12px] font-semibold", isHigh ? "text-bad" : "text-t3")}>
            {SEVERITY_LABEL[issue.severity]}
          </span>
          <span className="font-medium text-t1">{issue.title}</span>
          {issue.resolved && <span className="ml-auto text-[12px] text-ok">已解决</span>}
        </div>

        {/* §24 严重度分歧 / §25 无文件证据：只提示、不改变门控，帮助人工判断意见质量。 */}
        {(issue.severityDisagreement || issue.file == null) && (
          <div className="mt-1 flex flex-wrap gap-1.5 text-[11px]">
            {issue.severityDisagreement && (
              <span className="rounded-full bg-human-bg px-1.5 py-0.5 text-human" title="不同审查者对这条意见的严重程度判断不一致，建议人工裁决">
                ⚠ 严重度存在分歧
              </span>
            )}
            {issue.file == null && (
              <span className="rounded-full bg-raised px-1.5 py-0.5 text-t3" title="这条意见没有指向具体文件，无法直接对照代码，请人工核实">
                无文件定位，请人工核实
              </span>
            )}
          </div>
        )}

        {/* 每个问题显示由谁发现、几人同意 (05 §6.11)。 */}
        {(reporters.length > 0 || issue.agreementCount > 1) && (
          <div className="mt-1 flex flex-wrap items-center gap-1.5 text-[11px] text-t3">
            {reporters.length > 0 && (
              <>
                <span>由</span>
                {reporters.map((agent) => (
                  <span key={agent} className="flex items-center gap-1">
                    <AgentMark kind={agent} size={22} />
                    <span className="text-t2">{agentLabel(agent)}</span>
                  </span>
                ))}
                <span>发现</span>
              </>
            )}
            {issue.agreementCount > 1 && (
              <span className="rounded-full bg-raised px-1.5 py-0.5 text-t2">
                {memberCount ? `${issue.agreementCount}/${memberCount} 位成员同意` : `${issue.agreementCount} 人同意`}
                {reporters.length > 1 && " · 已合并重复报告"}
              </span>
            )}
          </div>
        )}
        {loc && (
          <button
            type="button"
            onClick={() => onJump?.(issue.file!, issue.lineStart)}
            disabled={!onJump}
            title="跳到 Diff 对应位置"
            className="mt-1 font-mono text-[12px] text-run hover:underline disabled:no-underline"
          >
            {loc}
          </button>
        )}
        {issue.description && <p className="mt-2 whitespace-pre-wrap text-[13px] leading-relaxed text-t2">{issue.description}</p>}
        {issue.suggestedAction && (
          <div className="mt-2">
            <button type="button" onClick={() => setOpen((o) => !o)} className="flex items-center gap-1 text-[12px] text-t3 hover:text-t2">
              <ChevronRight className={cn("size-3.5 transition-transform", open && "rotate-90")} /> 建议动作
            </button>
            {open && (
              <p className="mt-1 whitespace-pre-wrap rounded-md bg-app px-3 py-2 text-[13px] text-t2">{issue.suggestedAction}</p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
