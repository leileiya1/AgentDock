import { useState } from "react";
import { CheckCircle2, CircleAlert, RefreshCw, ShieldCheck } from "lucide-react";
import type { RollbackStrategy, TaskDetail } from "@/generated/bindings";
import { useDeliveryRefresh, useGovernance, useQualityReplay, useRollback, useRollbackPreflight } from "@/hooks/useGovernance";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { BudgetPanel } from "@/components/governance/BudgetPanel";
import { PermissionAuditPanel } from "@/components/permission/PermissionAuditPanel";
import { CiChecksPanel } from "@/components/governance/CiChecksPanel";
import { RollbackPreflightDialog } from "@/components/governance/RollbackPreflightDialog";
import { ReproducibilityPanel } from "@/components/governance/ReproducibilityPanel";
import { CopyText } from "@/components/CopyText";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { Button } from "@/components/ui/button";
import { rollbackAllowed } from "@/lib/governance/rollback";

export function GovernanceTab({ task, revision }: { task: TaskDetail; revision: number }) {
  const governance = useGovernance(task.id, revision);
  const replay = useQualityReplay();
  const refresh = useDeliveryRefresh();
  const rollback = useRollback();
  const [rollbackStrategy, setRollbackStrategy] = useState<RollbackStrategy | null>(null);
  const rollbackPreflight = useRollbackPreflight(task.id, rollbackStrategy !== null && task.status === "MERGED");

  if (governance.isLoading) return <div className="p-6"><SkeletonRows rows={6} /></div>;
  if (governance.isError || !governance.data) {
    return <div className="p-6"><ErrorState error={governance.error} onRetry={() => governance.refetch()} /></div>;
  }
  const data = governance.data;
  const budget = data.budget;
  const runReplay = async () => {
    try {
      const attempt = await replay.mutateAsync({ taskId: task.id, revision });
      if (attempt.status === "succeeded") toast.info("固定提交上的质量复验已通过");
      else if (attempt.status === "validation_failed") toast.error("复验已完成，但质量门禁未通过");
      else if (attempt.status === "drift_blocked") toast.error("检测到环境漂移，复验已在执行前停止");
      else toast.error("复验基础设施失败，请按治理页建议恢复");
    } catch (error) {
      toast.error(errorLine(error));
    }
  };
  const refreshDelivery = async () => {
    try { await refresh.mutateAsync(task.id); }
    catch (error) { toast.error(errorLine(error)); }
  };
  const confirmRollback = async () => {
    if (!rollbackStrategy) return;
    try {
      const latest = await rollbackPreflight.refetch();
      if (!latest.data || !rollbackAllowed(latest.data, rollbackStrategy)) {
        toast.error("Git 状态已经变化，请按预检提示处理后重试");
        return;
      }
      await rollback.mutateAsync({ taskId: task.id, strategy: rollbackStrategy });
      setRollbackStrategy(null);
      toast.info(rollbackStrategy === "undo" ? "合并已安全撤销" : "已创建回滚提交");
    } catch (error) {
      toast.error(errorLine(error));
    }
  };

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="mx-auto flex max-w-4xl flex-col gap-4">
        <BudgetPanel usage={budget} />
        <PermissionAuditPanel taskId={task.id} />

        <section className="rounded-[var(--radius-panel)] border border-line bg-panel/60 p-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <h2 className="font-semibold">质量评估</h2>
              <p className="mt-1 text-[12px] text-t3">在固定 revision commit 上重跑验证，不会再次调用开发 Agent。</p>
            </div>
            <Button
              variant="outline"
              disabled={revision <= 0 || !data.manifest || replay.isPending}
              title={!data.manifest ? "这个历史 revision 没有可复现运行清单，无法复验" : undefined}
              onClick={runReplay}
            >
              <RefreshCw className={`mr-1.5 size-3.5 ${replay.isPending ? "animate-spin" : ""}`} />
              可复现复验
            </Button>
          </div>
          {data.quality ? (
            <div className="mt-4 grid grid-cols-[120px_1fr] gap-4">
              <div className={`grid h-24 place-items-center rounded-xl border ${data.quality.passed ? "border-ok/40 bg-ok/5" : "border-human/40 bg-human-bg"}`}>
                <div className="text-center">
                  <div className="text-3xl font-semibold">{data.quality.score}</div>
                  <div className="text-[12px] text-t3">等级 {data.quality.grade}{data.quality.replay ? " · 复验" : ""}</div>
                </div>
              </div>
              <div className="grid grid-cols-2 gap-2">
                {data.quality.checks.map((check) => (
                  <div key={check.name} className="flex items-start gap-2 rounded-md border border-line bg-app/50 p-2.5">
                    {check.passed ? <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-ok" /> : <CircleAlert className="mt-0.5 size-4 shrink-0 text-human" />}
                    <div><div className="text-[12px] font-medium">{qualityLabel(check.name)} · {check.weight} 分</div><div className="mt-0.5 text-[11px] text-t3">{check.detail}</div></div>
                  </div>
                ))}
              </div>
            </div>
          ) : <p className="mt-4 text-[13px] text-t3">完成验证和独立审查后生成质量分。</p>}
        </section>

        <ReproducibilityPanel manifest={data.manifest} originalQuality={data.originalQuality} latestReplay={data.latestReplay} />

        <section className="rounded-[var(--radius-panel)] border border-line bg-panel/60 p-4">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h2 className="font-semibold">交付与回滚</h2>
              <p className="mt-1 text-[12px] text-t3">{deliveryLabel(task.policy.deliveryMode)}；CI 未通过时不会标记合并完成。</p>
            </div>
          </div>
          {data.delivery && (
            <div className="mt-3 flex flex-wrap items-center gap-3 text-[12px] text-t2">
              <span className="inline-flex items-center gap-1"><ShieldCheck className="size-4 text-ok" /> {data.delivery.state}</span>
              {data.delivery.ciStatus && <span>CI：{data.delivery.ciStatus}</span>}
              {data.delivery.remoteUrl && <a className="text-run hover:underline" href={data.delivery.remoteUrl} target="_blank" rel="noreferrer">打开请求 #{data.delivery.number ?? ""}</a>}
              {data.delivery.mergeCommit && <CopyText value={data.delivery.mergeCommit}>merge {data.delivery.mergeCommit.slice(0, 8)}</CopyText>}
            </div>
          )}
          {data.delivery && task.policy.deliveryMode !== "local_merge" && task.status !== "ROLLED_BACK" && (
            <CiChecksPanel delivery={data.delivery} refreshing={refresh.isPending} onRefresh={refreshDelivery} />
          )}
          {task.status === "MERGED" && (
            <div className="mt-4 flex justify-end gap-2 border-t border-line pt-3">
              <Button variant="outline" onClick={() => setRollbackStrategy("undo")}>撤销刚刚的本地合并</Button>
              <Button variant="danger" onClick={() => setRollbackStrategy("revert")}>创建回滚提交</Button>
            </div>
          )}
        </section>
      </div>

      <RollbackPreflightDialog
        open={rollbackStrategy !== null}
        strategy={rollbackStrategy}
        preflight={rollbackPreflight.data}
        loading={rollbackPreflight.isLoading || rollbackPreflight.isFetching}
        error={rollbackPreflight.isError}
        confirming={rollback.isPending}
        onClose={() => setRollbackStrategy(null)}
        onRetry={() => { void rollbackPreflight.refetch(); }}
        onConfirm={() => { void confirmRollback(); }}
      />
    </div>
  );
}

const qualityLabel = (name: string) => ({ validation: "自动验证", independent_review: "独立审查", high_risk_issues: "高风险问题", control_plane_changes: "控制面变更" }[name] ?? name);
const deliveryLabel = (mode?: string) => mode === "github_pr" ? "GitHub PR + CI" : mode === "gitlab_mr" ? "GitLab MR + CI" : "本地安全合并";
