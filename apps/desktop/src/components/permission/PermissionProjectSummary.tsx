import { ScrollText, ShieldAlert, ShieldCheck } from "lucide-react";
import type { TaskSummary } from "@/generated/bindings";
import { usePermissionRules } from "@/hooks/usePermissions";
import { summarizeRules } from "@/lib/permission/rules";
import { relativeTime } from "@/lib/format";

/**
 * 项目页权限概览 (06 §9 第 3 条)：当前权限模式、有效项目规则数、最近一次越界请求。
 * 产品化改版后作为 hero 内的一排紧凑 chip 呈现，与首页概览融为一体。
 * 「最近越界」从任务列表里 blockedReason === permission_required 的任务推导，无需额外后端调用。
 */
export function PermissionProjectSummary({
  projectId,
  tasks,
}: {
  projectId: string;
  tasks: TaskSummary[] | undefined;
}) {
  const rules = usePermissionRules(projectId);
  const summary = summarizeRules(rules.data);

  const latestBreach = (tasks ?? [])
    .filter((t) => t.blockedReason === "permission_required")
    .sort((a, b) => Date.parse(b.updatedAt) - Date.parse(a.updatedAt))[0];

  return (
    <span className="inline-flex flex-wrap items-center gap-1.5">
      <span className="inline-flex items-center gap-1 rounded-full border border-ok/30 bg-ok/5 px-2 py-0.5 text-[11px] font-medium text-ok">
        <ShieldCheck className="size-3" aria-hidden /> 自动执行 · 受限沙箱
      </span>
      <span className="inline-flex items-center gap-1 rounded-full border border-line bg-panel/70 px-2 py-0.5 text-[11px] text-t2">
        <ScrollText className="size-3" aria-hidden /> 规则 <span className="font-medium tabular-nums text-t1">{summary.active}</span>
      </span>
      {latestBreach && (
        <span className="inline-flex items-center gap-1 rounded-full border border-human/40 bg-human-bg px-2 py-0.5 text-[11px] font-medium text-human">
          <ShieldAlert className="size-3" aria-hidden /> 越界 {relativeTime(latestBreach.updatedAt)}
        </span>
      )}
    </span>
  );
}
