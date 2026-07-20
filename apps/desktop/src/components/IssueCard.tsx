import { useState } from "react";
import { ChevronRight } from "lucide-react";
import type { ReviewIssue, Severity } from "@/generated/bindings";
import { cn } from "@/lib/utils";

const SEVERITY_LABEL: Record<Severity, string> = { critical: "严重", high: "高", medium: "中", low: "低" };

function severityColor(sev: Severity): string {
  return sev === "critical" || sev === "high" ? "var(--color-bad)" : "var(--color-t3)";
}

interface Props {
  issue: ReviewIssue;
  onJump?: (file: string, line: number | null) => void;
}

export function IssueCard({ issue, onJump }: Props) {
  const [open, setOpen] = useState(false);
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
          {issue.agreementCount > 1 && <span className="rounded-full bg-raised px-1.5 py-0.5 text-[10px] text-t2">{issue.agreementCount} 人同意</span>}
          {issue.resolved && <span className="ml-auto text-[12px] text-ok">已解决</span>}
        </div>
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
