import { useState } from "react";
import type { TeamRecoveryView, TeamResourceState, TeamView } from "@/generated/bindings";
import { useTeamAction, useTeamRecovery } from "@/hooks/useTeam";
import { Button } from "@/components/ui/button";
import { errorLine } from "@/copy/errors";

const RESOURCE: Record<TeamResourceState, string> = { not_started: "尚未启动进程", present: "受管进程仍存在", stopped: "已确认停止", unknown: "进程结果未知 · 保留占用" };
export function TeamRecoveryFacts({ recovery }: { recovery: TeamRecoveryView }) {
  return <div className="mt-3 space-y-3" data-testid="team-recovery-facts">
    <p className="text-meta text-t2">后端检查时间 {recovery.inspectedAt} · 剩余尝试 {recovery.attemptsRemaining} · 剩余产品返工 {recovery.reworksRemaining}</p>
    {recovery.notices.length > 0 && <ul className="space-y-2 text-body">{recovery.notices.map((notice) => <li key={notice}>{notice}</li>)}</ul>}
    {recovery.resources.map((r) => <div key={r.attemptId} className="rounded-control border border-line bg-app p-3 text-meta">
      <p className="font-medium">{r.workItemId} · {r.memberId} · {r.provider ?? "程序 Runner"}</p><p>{RESOURCE[r.state]}</p><p className="mt-1 text-t2">{r.detail}</p><p className="mt-1 break-all text-t3">尝试 {r.attemptId} · {r.failureClass ?? "正在执行"}{r.cancelRequested ? " · 已请求停止" : ""}</p>
    </div>)}
    {recovery.resources.length === 0 && <p className="text-body">当前没有占用资源的受管尝试。此事实不代表任务已通过验收。</p>}
    {recovery.targetChanged && <p className="break-all text-meta">目标基线 {recovery.targetBase} → 当前分支 {recovery.targetCurrent}</p>}
  </div>;
}
export function TeamRecoveryPanel({ view }: { view: TeamView }) {
  const [opened, setOpened] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const relevant = ["blocked", "paused", "cancelled"].includes(view.phase);
  const query = useTeamRecovery(view.taskId, relevant || opened);
  const action = useTeamAction(view.taskId);
  if (!relevant && !opened) return <Button variant="ghost" onClick={() => setOpened(true)}>检查恢复状态与目标基线</Button>;
  const canReconcile = query.data?.resources.some((r) => r.cancelRequested || r.failureClass === "result_unknown") ?? false;
  return <section className="rounded-section border border-line p-4" data-testid="team-recovery-panel">
    <div className="flex flex-wrap items-center justify-between gap-2"><h2 className="text-heading">恢复检查与资源对账</h2><Button variant="ghost" size="sm" disabled={query.isFetching} onClick={() => void query.refetch()}>重新检查</Button></div>
    {query.isPending && <p className="mt-3 text-body">正在读取后端资源事实…</p>}
    {query.error && <p role="alert" className="mt-3 text-body">{errorLine(query.error)}</p>}
    {query.data && <TeamRecoveryFacts recovery={query.data} />}
    {canReconcile && <div className="mt-3 space-y-2"><Button variant="outline" disabled={action.isPending} onClick={async () => { setProblem(null); try { await action.mutateAsync({ action: "reconcile_resources" }); } catch (e) { setProblem(errorLine(e)); } }}>按已记录身份回收并对账</Button><p className="text-meta text-t3">后台只回收能核实身份的旧资源。无法确认时继续保留占用；对账不接受旧输出、不重置额度，也不会恢复已取消的团队。</p></div>}
    {problem && <p role="alert" className="mt-3 text-body">{problem}</p>}
  </section>;
}
