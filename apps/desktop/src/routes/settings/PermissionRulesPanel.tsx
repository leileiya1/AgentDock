import { useState } from "react";
import { ScrollText } from "lucide-react";
import type { PermissionRule } from "@/generated/bindings";
import { usePermissionRules, useRevokeRule } from "@/hooks/usePermissions";
import { actionCopy, roleLabel } from "@/copy/permission";
import { agentLabel } from "@/copy/agents";
import { RULE_STATUS_COPY, ruleStatus, sortRules } from "@/lib/permission/rules";
import { rulePreview } from "@/lib/permission/rules";
import { relativeTime } from "@/lib/format";
import { Dialog } from "@/components/Dialog";
import { Button } from "@/components/ui/button";
import { PermissionPill } from "@/components/permission/PermissionBadge";
import { SkeletonRows } from "@/components/Skeleton";
import { ErrorState } from "@/components/ErrorState";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";

/**
 * 项目规则管理 (06 §9 第 17/18/19 条)。列出精确规则、最近命中、到期时间与状态，并提供撤销。
 * 撤销调用真实 permission_rule_revoke——不杀死已完成动作，但阻止下一次执行。撤销为不可逆的
 * 高风险操作，二次确认。
 */
export function PermissionRulesPanel({ projectId }: { projectId: string }) {
  const rules = usePermissionRules(projectId);
  const revoke = useRevokeRule(projectId);
  const [confirm, setConfirm] = useState<PermissionRule | null>(null);

  const doRevoke = async () => {
    if (!confirm) return;
    try {
      await revoke.mutateAsync(confirm.id);
      toast.info("已撤销规则，下一次相同请求会重新询问");
      setConfirm(null);
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  return (
    <div className="rounded-section border border-line bg-app p-3">
      <div className="mb-2 flex items-center gap-2 font-semibold">
        <ScrollText className="size-4 text-t2" /> 项目权限规则
      </div>

      {rules.isError ? (
        <ErrorState error={rules.error} onRetry={() => rules.refetch()} compact />
      ) : rules.isLoading || !rules.data ? (
        <SkeletonRows rows={2} />
      ) : rules.data.length === 0 ? (
        <p className="text-meta text-t3">还没有保存任何项目规则。授权时选择「保存项目规则」会在这里出现。</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {sortRules(rules.data).map((rule) => (
            <RuleRow key={rule.id} rule={rule} onRevoke={() => setConfirm(rule)} />
          ))}
        </ul>
      )}

      <Dialog
        open={!!confirm}
        onClose={() => setConfirm(null)}
        title="撤销项目规则"
        onConfirmKey={doRevoke}
        footer={
          <>
            <Button variant="outline" onClick={() => setConfirm(null)}>取消</Button>
            <Button variant="danger" onClick={doRevoke} disabled={revoke.isPending}>确认撤销</Button>
          </>
        }
      >
        <p className="text-body text-t2">
          撤销不会影响已经完成的动作，但下一次相同的请求会重新询问你。此操作不可撤销。
        </p>
        {confirm && (
          <div className="mt-3 rounded-control border border-line bg-raised/40 p-2.5">
            <dl className="flex flex-col gap-1">
              {rulePreview(confirm).slice(0, 4).map((line, i) => (
                <div key={i} className="flex flex-wrap items-baseline gap-x-2 text-meta">
                  <dt className="shrink-0 text-t3">{line.label}</dt>
                  <dd className={line.mono ? "min-w-0 break-all font-mono text-t1" : "text-t1"}>{line.value}</dd>
                </div>
              ))}
            </dl>
          </div>
        )}
      </Dialog>
    </div>
  );
}

export function RuleRow({ rule, onRevoke }: { rule: PermissionRule; onRevoke: () => void }) {
  const status = ruleStatus(rule);
  const statusCopy = RULE_STATUS_COPY[status];
  const action = actionCopy(rule.actionType);
  const argv = rule.operation.argv.join(" ");
  const domains = rule.operation.networkDomains.join("、");

  return (
    <li className="flex flex-col gap-1.5 rounded-control border border-line bg-raised/30 px-3 py-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
          <span className="text-body font-medium text-t1">{action.label}</span>
          <span className="rounded bg-raised px-1.5 py-0.5 text-micro text-t3">
            {agentLabel(rule.providerId)} · {roleLabel(rule.role)}
          </span>
          <PermissionPill tone={statusCopy.tone === "ok" ? "ok" : statusCopy.tone === "bad" ? "bad" : "idle"}>
            {statusCopy.label}
          </PermissionPill>
        </div>
        {status === "active" && (
          <Button variant="danger" size="sm" onClick={onRevoke}>撤销</Button>
        )}
      </div>
      {argv && <code className="block max-w-full overflow-x-auto whitespace-pre font-mono text-meta text-t2">{argv}</code>}
      {domains && <span className="font-mono text-meta text-t2">{domains}</span>}
      <div className="flex flex-wrap gap-x-3 gap-y-0.5 text-meta text-t3">
        <span>最近命中：{rule.lastMatchedAt ? relativeTime(rule.lastMatchedAt) : "从未"}</span>
        <span>到期：{rule.expiresAt ? relativeTime(rule.expiresAt) : "长期有效"}</span>
        <span>创建：{relativeTime(rule.createdAt)}</span>
      </div>
    </li>
  );
}
