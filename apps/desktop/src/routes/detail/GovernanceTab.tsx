import { useState } from "react";
import { CheckCircle2, CircleAlert, RefreshCw, ShieldCheck } from "lucide-react";
import type { RollbackStrategy, TaskDetail } from "@/generated/bindings";
import { useDeliveryRefresh, useGovernance, useQualityReplay, useRollback } from "@/hooks/useGovernance";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { Dialog } from "@/components/Dialog";
import { CopyText } from "@/components/CopyText";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { Button } from "@/components/ui/button";

export function GovernanceTab({ task, revision }: { task: TaskDetail; revision: number }) {
  const governance = useGovernance(task.id, revision);
  const replay = useQualityReplay();
  const refresh = useDeliveryRefresh();
  const rollback = useRollback();
  const [rollbackStrategy, setRollbackStrategy] = useState<RollbackStrategy | null>(null);

  if (governance.isLoading) return <div className="p-6"><SkeletonRows rows={6} /></div>;
  if (governance.isError || !governance.data) {
    return <div className="p-6"><ErrorState error={governance.error} onRetry={() => governance.refetch()} /></div>;
  }
  const data = governance.data;
  const budget = data.budget;
  const runReplay = async () => {
    try {
      await replay.mutateAsync({ taskId: task.id, revision });
      toast.info("固定提交上的质量复验已完成");
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
        <section className="grid grid-cols-3 gap-3">
          <BudgetCard label="Token" used={budget.tokensUsed} limit={budget.tokenBudget} />
          <BudgetCard label="费用" used={budget.costUsd ?? 0} limit={budget.costBudgetUsd} prefix="$" digits={4} />
          <BudgetCard label="运行时间" used={budget.timeUsedSecs} limit={budget.timeBudgetSecs} suffix=" 秒" />
        </section>

        <section className="rounded-[var(--radius-panel)] border border-line bg-panel/60 p-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <h2 className="font-semibold">质量评估</h2>
              <p className="mt-1 text-[12px] text-t3">在固定 revision commit 上重跑验证，不会再次调用开发 Agent。</p>
            </div>
            <Button variant="outline" disabled={revision <= 0 || replay.isPending} onClick={runReplay}>
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

        <section className="rounded-[var(--radius-panel)] border border-line bg-panel/60 p-4">
          <h2 className="font-semibold">可复现运行清单</h2>
          {data.manifest ? (
            <div className="mt-3 grid grid-cols-2 gap-x-6 gap-y-2 text-[12px]">
              <ManifestRow label="commit" value={data.manifest.commitSha} />
              <ManifestRow label="manifest" value={data.manifest.manifestSha256} />
              <ManifestRow label="输入" value={data.manifest.inputSha256} />
              <ManifestRow label="代码差异" value={data.manifest.patchSha256} />
              <ManifestRow label="验证配置" value={data.manifest.validationConfigSha256} />
              <div className="flex gap-2"><span className="w-16 text-t3">验证环境</span><span className="text-t2">{data.manifest.environment["validation_location"] === "remote" ? "远程" : "本机"} · {data.manifest.environment["validation_platform"] ?? "未知"}</span></div>
            </div>
          ) : <p className="mt-3 text-[13px] text-t3">首轮验证完成后生成。</p>}
        </section>

        <section className="rounded-[var(--radius-panel)] border border-line bg-panel/60 p-4">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h2 className="font-semibold">交付与回滚</h2>
              <p className="mt-1 text-[12px] text-t3">{deliveryLabel(task.policy.deliveryMode)}；CI 未通过时不会标记合并完成。</p>
            </div>
            {task.policy.deliveryMode !== "local_merge" && task.status !== "ROLLED_BACK" && (
              <Button variant="outline" disabled={refresh.isPending} onClick={refreshDelivery}>刷新 PR / CI</Button>
            )}
          </div>
          {data.delivery && (
            <div className="mt-3 flex flex-wrap items-center gap-3 text-[12px] text-t2">
              <span className="inline-flex items-center gap-1"><ShieldCheck className="size-4 text-ok" /> {data.delivery.state}</span>
              {data.delivery.ciStatus && <span>CI：{data.delivery.ciStatus}</span>}
              {data.delivery.remoteUrl && <a className="text-run hover:underline" href={data.delivery.remoteUrl} target="_blank" rel="noreferrer">打开请求 #{data.delivery.number ?? ""}</a>}
              {data.delivery.mergeCommit && <CopyText value={data.delivery.mergeCommit}>merge {data.delivery.mergeCommit.slice(0, 8)}</CopyText>}
            </div>
          )}
          {task.status === "MERGED" && (
            <div className="mt-4 flex justify-end gap-2 border-t border-line pt-3">
              <Button variant="outline" onClick={() => setRollbackStrategy("undo")}>撤销刚刚的本地合并</Button>
              <Button variant="danger" onClick={() => setRollbackStrategy("revert")}>创建回滚提交</Button>
            </div>
          )}
        </section>
      </div>

      <Dialog
        open={rollbackStrategy !== null}
        onClose={() => setRollbackStrategy(null)}
        title={rollbackStrategy === "undo" ? "确认撤销合并" : "确认创建回滚提交"}
        footer={<><Button variant="outline" onClick={() => setRollbackStrategy(null)}>取消</Button><Button variant="danger" disabled={rollback.isPending} onClick={confirmRollback}>确认执行</Button></>}
      >
        <p className="text-[13px] leading-relaxed text-t2">
          {rollbackStrategy === "undo"
            ? "仅当目标分支仍停在本任务的 merge commit 且工作区干净时才会执行；检测到后续提交会拒绝，绝不会覆盖别人工作。"
            : "不会改写已有历史，而是针对本任务 merge commit 创建一个新的反向提交，适合已经共享或继续开发的分支。"}
        </p>
      </Dialog>
    </div>
  );
}

function BudgetCard({ label, used, limit, prefix = "", suffix = "", digits = 0 }: { label: string; used: number; limit?: number | null; prefix?: string; suffix?: string; digits?: number }) {
  const ratio = limit ? Math.min(100, (used / limit) * 100) : 0;
  const format = (value: number) => `${prefix}${value.toFixed(digits)}${suffix}`;
  return <div className="rounded-[var(--radius-panel)] border border-line bg-panel/60 p-3"><div className="flex justify-between text-[12px]"><span className="font-medium">{label}</span><span className="text-t3">{format(used)} / {limit == null ? "不限" : format(limit)}</span></div><div className="mt-2 h-1.5 overflow-hidden rounded-full bg-line"><div className={`h-full rounded-full ${ratio >= 90 ? "bg-human" : "bg-run"}`} style={{ width: `${ratio}%` }} /></div></div>;
}

function ManifestRow({ label, value }: { label: string; value: string }) {
  return <div className="flex min-w-0 gap-2"><span className="w-16 shrink-0 text-t3">{label}</span><CopyText value={value} className="truncate font-mono text-t2">{value.slice(0, 16)}…</CopyText></div>;
}

const qualityLabel = (name: string) => ({ validation: "自动验证", independent_review: "独立审查", high_risk_issues: "高风险问题", control_plane_changes: "控制面变更" }[name] ?? name);
const deliveryLabel = (mode?: string) => mode === "github_pr" ? "GitHub PR + CI" : mode === "gitlab_mr" ? "GitLab MR + CI" : "本地安全合并";
