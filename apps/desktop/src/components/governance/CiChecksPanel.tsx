import {
  CheckCircle2,
  CircleAlert,
  CircleMinus,
  CircleQuestionMark,
  Clock3,
  ExternalLink,
  RefreshCw,
} from "lucide-react";
import type { CiCheck, CiCheckStatus, DeliveryRecord } from "@/generated/bindings";
import { absoluteTime } from "@/lib/format";
import { CI_STATUS_LABEL, ciCheckSummary, ciCounts, ciDuration } from "@/lib/governance/ci";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

export function CiChecksPanel({
  delivery,
  refreshing,
  onRefresh,
}: {
  delivery: DeliveryRecord;
  refreshing: boolean;
  onRefresh: () => void;
}) {
  const checks = delivery.ciChecks ?? [];
  const counts = ciCounts(checks);

  return (
    <div className="mt-4 border-t border-line pt-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h3 className="text-body font-medium">CI 检查</h3>
          <p className="mt-0.5 text-meta text-t3">
            {checks.length
              ? `${counts.passed}/${counts.total} 通过${counts.failed ? ` · ${counts.failed} 失败` : ""}${counts.pending ? ` · ${counts.pending} 运行中` : ""}`
              : "尚未收到 Check；状态未知不会被当作通过。"}
            {` · 最近刷新 ${absoluteTime(delivery.updatedAt)}`}
          </p>
        </div>
        <Button variant="outline" size="sm" disabled={refreshing} onClick={onRefresh}>
          <RefreshCw className={cn("size-3.5", refreshing && "animate-spin")} />
          刷新 CI
        </Button>
      </div>

      {checks.length === 0 ? (
        <p className="mt-3 rounded-control border border-line bg-app/40 px-3 py-2 text-meta leading-relaxed text-t2">
          GitHub/GitLab 还没有返回检查明细。如果仓库未配置必需 Check，AgentFlow 会继续等待，不会误判为可以合并。
        </p>
      ) : (
        <div className="mt-3 divide-y divide-line overflow-hidden rounded-control border border-line bg-app/35">
          {checks.map((check, index) => (
            <CiCheckRow key={`${check.workflow ?? "ci"}:${check.name}:${index}`} check={check} />
          ))}
        </div>
      )}
    </div>
  );
}

function CiCheckRow({ check }: { check: CiCheck }) {
  const summary = ciCheckSummary(check);
  const duration = ciDuration(check);
  const Icon = STATUS_ICON[check.status];
  return (
    <div className="flex items-start gap-3 px-3 py-2.5">
      <Icon className={cn("mt-0.5 size-4 shrink-0", STATUS_TONE[check.status])} aria-hidden />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-meta">
          <span className="font-medium text-t1">{check.name}</span>
          <span className={cn("font-medium", STATUS_TONE[check.status])}>{CI_STATUS_LABEL[check.status]}</span>
          <span className="rounded border border-line px-1.5 py-0.5 text-micro text-t3">
            {check.required ? "必需" : "可选"}
          </span>
          {check.workflow && <span className="text-t3">{check.workflow}</span>}
          {duration && <span className="text-t3">{duration}</span>}
        </div>
        {summary && (
          <p className={cn("mt-1 break-words text-meta leading-relaxed", check.status === "failed" ? "text-bad" : "text-t3")}>{summary}</p>
        )}
      </div>
      {check.detailsUrl && (
        <a
          className="inline-flex shrink-0 items-center gap-1 text-meta text-link hover:underline"
          href={check.detailsUrl}
          target="_blank"
          rel="noreferrer"
          aria-label={`打开 ${check.name} 的远端日志`}
        >
          远端日志 <ExternalLink className="size-3" aria-hidden />
        </a>
      )}
    </div>
  );
}

const STATUS_ICON: Record<CiCheckStatus, typeof CheckCircle2> = {
  passed: CheckCircle2,
  failed: CircleAlert,
  cancelled: CircleMinus,
  pending: Clock3,
  skipped: CircleMinus,
  unknown: CircleQuestionMark,
};

const STATUS_TONE: Record<CiCheckStatus, string> = {
  passed: "text-ok",
  failed: "text-bad",
  cancelled: "text-status-idle",
  pending: "text-status-running",
  skipped: "text-t3",
  unknown: "text-t3",
};
