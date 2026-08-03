import { useMemo, useState } from "react";
import { ClipboardList, Filter } from "lucide-react";
import type { PermissionRequest } from "@/generated/bindings";
import { usePermissionRequests } from "@/hooks/usePermissions";
import { actionCopy, roleLabel } from "@/copy/permission";
import { agentLabel } from "@/copy/agents";
import {
  auditFacets,
  auditLine,
  EMPTY_FILTER,
  filterRequests,
  isActiveFilter,
  type AuditDecision,
  type AuditFilter,
} from "@/lib/permission/audit";
import { relativeTime } from "@/lib/format";
import { RequestStatusBadge } from "@/components/permission/PermissionBadge";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { SkeletonRows } from "@/components/Skeleton";
import { ErrorState } from "@/components/ErrorState";

const DECISIONS: { value: AuditDecision; label: string }[] = [
  { value: "all", label: "全部决定" },
  { value: "pending", label: "请求（待处理）" },
  { value: "approved", label: "允许" },
  { value: "denied", label: "拒绝" },
  { value: "cancelled", label: "取消" },
  { value: "expired", label: "过期" },
];

/**
 * 权限审计 (06 §9 第 20/21 条)。可按 Provider、动作类型、决定、规则筛选当前任务的权限活动；
 * 默认每行只显示「请求/允许/拒绝」的简短文字，技术 JSON（规范化操作、SHA、命中规则）折叠在
 * 「高级详情」里，用原生 details 保证键盘与读屏可达。
 */
export function PermissionAuditPanel({ taskId }: { taskId: string }) {
  const requests = usePermissionRequests(taskId);
  const [filter, setFilter] = useState<AuditFilter>(EMPTY_FILTER);

  const all = requests.data ?? [];
  const facets = useMemo(() => auditFacets(all), [all]);
  const rows = useMemo(() => filterRequests(all, filter), [all, filter]);

  return (
    <section className="rounded-section border border-line bg-panel/60 p-4">
      <div className="mb-3 flex items-center gap-2">
        <ClipboardList className="size-4 text-t2" aria-hidden />
        <h2 className="font-semibold">权限审计</h2>
      </div>

      {requests.isError ? (
        <ErrorState error={requests.error} onRetry={() => requests.refetch()} compact />
      ) : requests.isLoading ? (
        <SkeletonRows rows={3} />
      ) : all.length === 0 ? (
        <p className="text-meta text-t3">这个任务还没有权限请求记录。</p>
      ) : (
        <>
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <Filter className="size-3.5 text-t3" aria-hidden />
            <FacetSelect
              label="Provider"
              value={filter.provider}
              options={facets.providers.map((p) => ({ value: p, label: agentLabel(p) }))}
              onChange={(v) => setFilter((f) => ({ ...f, provider: v }))}
              allLabel="全部 Provider"
            />
            <FacetSelect
              label="动作类型"
              value={filter.actionType}
              options={facets.actionTypes.map((a) => ({ value: a, label: actionCopy(a).label }))}
              onChange={(v) => setFilter((f) => ({ ...f, actionType: v as AuditFilter["actionType"] }))}
              allLabel="全部动作"
            />
            <FacetSelect
              label="决定"
              value={filter.decision}
              options={DECISIONS.slice(1)}
              onChange={(v) => setFilter((f) => ({ ...f, decision: v as AuditDecision }))}
              allLabel="全部决定"
            />
            <FacetSelect
              label="规则"
              value={filter.rule}
              options={[
                { value: "matched", label: "命中规则" },
                { value: "none", label: "未命中规则" },
              ]}
              onChange={(v) => setFilter((f) => ({ ...f, rule: v as AuditFilter["rule"] }))}
              allLabel="不限规则"
            />
            {isActiveFilter(filter) && (
              <Button variant="ghost" size="sm" onClick={() => setFilter(EMPTY_FILTER)}>
                清除筛选
              </Button>
            )}
            <span className="ml-auto text-meta text-t3 tabular-nums">
              {rows.length} / {all.length} 条
            </span>
          </div>

          {rows.length === 0 ? (
            <p className="text-meta text-t3">没有符合筛选条件的记录。</p>
          ) : (
            <ul className="flex flex-col gap-1.5">
              {rows.map((r) => (
                <AuditRow key={r.id} request={r} />
              ))}
            </ul>
          )}
        </>
      )}
    </section>
  );
}

function FacetSelect<T extends string>({
  label,
  value,
  options,
  onChange,
  allLabel,
}: {
  label: string;
  value: T | "all";
  options: { value: T; label: string }[];
  onChange: (value: T | "all") => void;
  allLabel: string;
}) {
  return (
    <Select value={value} onValueChange={(v) => onChange(v as T | "all")}>
      <SelectTrigger className="h-7 w-auto gap-1 text-meta" aria-label={label}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">{allLabel}</SelectItem>
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value}>
            {o.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export function AuditRow({ request }: { request: PermissionRequest }) {
  return (
    <li className="rounded-control border border-line bg-app px-3 py-2">
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <RequestStatusBadge status={request.status} />
        <span className="min-w-0 flex-1 truncate text-body text-t1">{auditLine(request)}</span>
        <span className="shrink-0 text-meta text-t3">
          {agentLabel(request.providerId)} · {roleLabel(request.role)}
        </span>
        <span className="shrink-0 text-meta text-t3">{relativeTime(request.decidedAt ?? request.requestedAt)}</span>
      </div>
      {/* 技术 JSON 放高级详情，默认视图保持简短 (§9 第 21 条)；details 原生键盘/读屏可达。 */}
      <details className="mt-1 group">
        <summary className="cursor-pointer list-none text-meta text-t3 hover:text-t2">
          <span className="group-open:hidden">▸ 高级详情</span>
          <span className="hidden group-open:inline">▾ 高级详情</span>
        </summary>
        <div className="mt-1.5 flex flex-col gap-1 text-meta text-t3">
          <div className="flex flex-wrap gap-x-3">
            <span>operation_sha256：<span className="font-mono">{request.operationSha256}</span></span>
            <span>policy_sha256：<span className="font-mono">{request.policySha256}</span></span>
          </div>
          <div className="flex flex-wrap gap-x-3">
            <span>revision：{request.revision}</span>
            <span>命中规则：{request.matchedRuleId ?? "无"}</span>
            <span>请求次数：{request.requestCount}</span>
          </div>
          <pre className="max-w-full overflow-x-auto rounded bg-raised/50 p-2 font-mono text-meta text-t2">
            {JSON.stringify(request.operation, null, 2)}
          </pre>
        </div>
      </details>
    </li>
  );
}
