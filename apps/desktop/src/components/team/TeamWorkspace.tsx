import { commands } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { useState } from "react";
import type { TeamAction, TeamView, TeamPhase, TeamCriterion, CriterionStatus } from "@/generated/bindings";
import { useTeamAction } from "@/hooks/useTeam";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/input";
import { errorLine } from "@/copy/errors";
import { ProviderSelect } from "./TeamTaskForm";
import { toast } from "@/stores/toastStore";
import { TeamRecoveryPanel } from "./TeamRecoveryPanel";
import { TeamPlanEditor } from "./TeamPlanEditor";
import { open } from "@tauri-apps/plugin-dialog";
const PHASE: Record<TeamPhase, string> = { draft: "草稿", planning: "主规划中", plan_approval: "等待计划确认", running: "团队执行中", paused: "已暂停派发", blocked: "需要处理", ready_for_delivery: "已验收 · 待交付", delivering: "交付对账中", delivered: "已交付", cancelled: "已取消" };
const STATUS: Record<CriterionStatus, string> = { not_run: "未测", passed: "通过", failed: "失败", blocked: "环境阻塞", inconclusive: "无法判断" };
const WORK: Record<string, string> = { pending: "待调度", active: "执行中", submitted: "产物检查中", output_accepted: "产物已接受", blocked: "阻塞", cancelled: "已取消", superseded: "已替换" };
export function TeamFacts({ view }: { view: TeamView }) {
  const [rawEvidence, setRawEvidence] = useState<Record<string, string>>({});
  return <div className="flex flex-col gap-5" data-testid="team-facts">
    <section className="rounded-section border border-line p-4"><h2 className="text-heading">成员与真实运行</h2><p className="mb-3 text-meta text-t3">席位 {view.config.members.length}/{view.config.teamCapacity} · 同时运行上限 {view.config.maxConcurrentAttempts} · 尝试 {view.attemptsUsed}/{view.config.maxAttempts} · 返工 {view.reworksUsed}/{view.config.maxReworks}</p>
      <div className="grid gap-2 sm:grid-cols-2">{view.config.members.map((m) => { const active = view.attempts.find((a) => a.identity.memberId === m.id && a.status === "RUNNING"); return <div className="rounded-control bg-app p-3" key={m.id}><div className="font-medium">{m.name}</div><div className="text-meta text-t2">{m.roles.join(" / ")} · {m.providers.join(" → ")}</div><div className="mt-1 text-body">{active ? `正在执行 ${active.identity.workItemId} · ${active.provider}` : "当前无受管运行"}</div></div>; })}</div>
      <p className="mt-3 text-meta text-t3">未报告的费用和 Token 为未知。Provider、账号和全局并发限制也会影响启动。</p>
    </section>
    <section className="rounded-section border border-line p-4"><h2 className="text-heading">工作项</h2><div className="mt-3 divide-y divide-line">{view.items.map((w) => <div key={w.spec.id} className="py-3"><div className="flex flex-wrap items-center justify-between gap-2"><strong className="text-body">{w.spec.title}</strong><span className="text-meta">{WORK[w.state] ?? w.state}</span></div><p className="text-meta text-t2">{w.spec.id} · {w.spec.kind} · 候选成员 {w.spec.memberIds.join(", ") || "程序 Runner"}</p><p className="text-meta text-t3">依赖：{w.spec.dependsOn.join(", ") || "无显式依赖"}{w.spec.kind === "implement" ? "；另需独立覆盖核对通过" : ""}</p>{w.detail && <p className="mt-1 whitespace-pre-wrap text-body">{w.detail}</p>}<details className="mt-1 text-meta"><summary>契约与版本</summary><p className="whitespace-pre-wrap">{w.spec.instructions}</p><code className="break-all">spec {w.specHash} · epoch {w.epoch}</code></details></div>)}</div></section>
    <section className="rounded-section border border-line p-4"><h2 className="text-heading">原始要求与覆盖</h2><p className="mt-2 whitespace-pre-wrap text-body">{view.originalGoal}</p><ul className="mt-3 space-y-2">{view.requirements.map((r) => <li key={r.id} className="text-body"><strong>{r.id}</strong> · {r.originalText}<div className="text-meta text-t3">验收映射：{view.criteria.filter((c) => c.criterion.requirementIds.includes(r.id)).map((c) => c.criterion.id).join(", ") || "尚无映射"}</div></li>)}</ul></section>
    <section className="rounded-section border border-line p-4"><h2 className="text-heading">逐条验收证据</h2><p className="mt-1 break-all text-meta text-t2">当前候选：{view.candidate ? `${view.candidate.id} · tree ${view.candidate.treeSha}` : "尚未完成集成"}</p>
      {view.criteria.length === 0 && <p className="mt-3 text-body">尚未形成验收契约。零检查不会通过。</p>}
      {view.criteria.map((c) => <div key={c.criterion.id} className="mt-3 border-t border-line pt-3" data-testid={`criterion-${c.criterion.id}`}><div className="flex justify-between gap-2"><strong className="text-body">{c.criterion.text}</strong><span className={`text-meta ${c.status === "passed" && !c.stale ? "text-status-success" : "text-t2"}`}>{c.stale ? "证据已失效" : STATUS[c.status]}</span></div><p className="mt-1 text-meta text-t2">预期：{c.criterion.expected} · {c.criterion.required ? "必需" : "可选"}</p>
        <details className="mt-2 text-meta"><summary>查看 {c.evidence.length} 条实际执行记录</summary>{c.evidence.map((e) => <div key={e.id} className="mt-2 break-all rounded-control bg-app p-2"><p>{STATUS[e.status]} · {e.checkId} · {e.source}</p><p>候选 {e.candidateId} · 成员 {e.memberId}</p><p>{e.detail}</p><p>原始记录：{e.rawPath}</p><p>SHA256 {e.rawSha256}</p><Button variant="outline" size="sm" onClick={() => unwrap(commands.teamEvidenceRead({ taskId: view.taskId, evidenceId: e.id })).then((raw) => setRawEvidence((v) => ({ ...v, [e.id]: raw }))).catch((error) => toast.error(errorLine(error)))}>查看原始输出</Button>{rawEvidence[e.id] && <pre className="mt-2 max-h-80 overflow-auto whitespace-pre-wrap">{rawEvidence[e.id]}</pre>}</div>)}</details>
      </div>)}
    </section>
    {view.gateBlockers.length > 0 && <section className="rounded-section border border-line p-4"><h2 className="text-heading">尚未满足的门禁</h2><ul className="mt-2 space-y-1 text-body">{view.gateBlockers.map((b, i) => <li key={`${i}-${b}`}>{b}</li>)}</ul></section>}
    <details className="rounded-section border border-line p-4"><summary>尝试历史、工作区与报告</summary>{view.attempts.map((a) => <div key={a.identity.attemptId} className="mt-3 break-all text-meta"><p>{a.identity.workItemId} / {a.identity.memberId} / {a.provider} · {a.status}</p><p>尝试 {a.identity.attemptId} · 代次 {a.identity.leaseEpoch}</p><p>工作区 {a.worktree ?? "尚未准备"}</p><p>输入 {a.inputBaseSha ?? "未知"} · {a.failureClass ?? "未记录故障"}</p><p>费用 {a.costUsd == null ? "未知" : `$${a.costUsd}`} · 输入 Token {a.tokensIn ?? "未知"} · 输出 Token {a.tokensOut ?? "未知"}</p></div>)}<p className="mt-3 break-all text-meta">报告 {view.reportPath ?? "尚未生成"}</p></details>
    <details className="text-meta text-t3"><summary>当前保证和适用边界</summary><ul className="mt-2 space-y-2">{view.limitations.map((l) => <li key={l}>{l}</li>)}</ul></details>
  </div>;
}
export function TeamWorkspace({ view, title }: { view: TeamView; title: string }) {
  const action = useTeamAction(view.taskId); const [problem, setProblem] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false); const [exportPath, setExportPath] = useState<string | null>(null);
  const [acceptanceText, setAcceptanceText] = useState(""); const [acceptanceReason, setAcceptanceReason] = useState("");
  const [replacement, setReplacement] = useState<Record<string, string>>({});
  const run = async (input: TeamAction) => { setProblem(null); try { await action.mutateAsync(input); } catch (e) { setProblem(errorLine(e)); } };
  const editable = !["cancelled", "delivered", "delivering"].includes(view.phase);
  const exportReport = async () => {
    setProblem(null); setExporting(true);
    try {
      const parentDirectory = await open({ directory: true, multiple: false, title: "选择验收报告保存位置" });
      if (typeof parentDirectory !== "string") return;
      const exported = await unwrap(commands.teamReportExport({ taskId: view.taskId, parentDirectory }));
      setExportPath(exported.directory); toast.info(`已导出报告与 ${exported.evidenceCount} 条证据记录`);
    } catch (e) { setProblem(errorLine(e)); }
    finally { setExporting(false); }
  };
  return <div className="h-full overflow-y-auto p-5" data-testid="team-workspace"><div className="mx-auto flex max-w-5xl flex-col gap-4">
    <header className="flex flex-wrap items-start justify-between gap-3"><div><h1 className="text-title">{title}</h1><p className="mt-1 text-body" role="status">{PHASE[view.phase]}</p><p className="text-meta text-t3">计划 v{view.planRevision} · 后端事实自动更新</p></div><div className="flex flex-wrap gap-2">
      {view.phase === "draft" && <Button variant="primary" disabled={action.isPending} onClick={() => run({ action: "start" })}>启动团队</Button>}
      {view.phase === "plan_approval" && <Button variant="primary" disabled={action.isPending} onClick={() => run({ action: "approve_plan", expected_revision: view.planRevision })}>确认当前计划</Button>}
      {["running", "planning", "plan_approval"].includes(view.phase) && <Button variant="outline" disabled={action.isPending} onClick={() => run({ action: "pause" })}>暂停派发</Button>}
      {["paused", "blocked"].includes(view.phase) && <Button variant="outline" disabled={action.isPending} onClick={() => run({ action: "resume" })}>恢复 / 重检</Button>}
      {editable && <Button variant="outline" disabled={action.isPending} onClick={() => run({ action: "cancel" })}>停止并取消团队</Button>}
      {view.phase === "ready_for_delivery" && <Button variant="primary" disabled={action.isPending || view.gateBlockers.length > 0} onClick={() => run({ action: "deliver" })}>批准并本地交付</Button>}
      {view.phase === "delivering" && <Button variant="outline" disabled={action.isPending} onClick={() => run({ action: "deliver" })}>对账交付结果</Button>}
      <Button variant="ghost" onClick={() => navigator.clipboard.writeText(JSON.stringify(view, null, 2)).then(() => toast.info("已复制后端验收报告")).catch(() => toast.error("复制失败，请查看报告文件"))}>复制报告</Button>
      <Button variant="outline" disabled={exporting} onClick={() => void exportReport()}>导出验收与证据</Button>
    </div></header>
    {view.detail && <p className="whitespace-pre-wrap rounded-control border border-line bg-app p-3 text-body">{view.detail}</p>}
    {problem && <p role="alert" className="rounded-control border border-line p-3 text-body">{problem}</p>}
    {exportPath && <p className="break-all rounded-control border border-line p-3 text-body">报告已保存：{exportPath}。manifest.json 记录快照版本、原始证据摘要及脱敏展示文件摘要；导出不改变任务验收状态。</p>}
    <TeamRecoveryPanel view={view} />
    <TeamFacts view={view} />
    {editable && <details className="rounded-section border border-line p-4"><summary>子任务控制与成员替换</summary><Button variant="outline" className="mt-3" disabled={action.isPending || view.attempts.some((a) => a.status === "RUNNING")} onClick={() => run({ action: "refresh_target" })}>更新目标分支基线并重新验证</Button><div className="mt-3 space-y-3">{view.items.filter((w) => ["blocked", "cancelled", "active", "pending"].includes(w.state)).map((w) => <div className="flex flex-wrap justify-between gap-2" key={w.spec.id}><span className="text-body">{w.spec.title}</span><div className="flex gap-2">{["blocked", "cancelled"].includes(w.state) && <Button variant="outline" size="sm" disabled={action.isPending} onClick={() => run({ action: "retry_work", work_item_id: w.spec.id })}>恢复此项</Button>}{["active", "pending"].includes(w.state) && <Button variant="ghost" size="sm" disabled={action.isPending} onClick={() => run({ action: "cancel_work", work_item_id: w.spec.id })}>取消此项</Button>}</div></div>)}
      {view.config.members.map((m) => <div key={m.id} className="flex flex-wrap items-center gap-2"><span className="min-w-28 text-body">{m.name}</span><ProviderSelect value={replacement[m.id] ?? m.providers[0]} onChange={(v) => v && setReplacement((r) => ({ ...r, [m.id]: v }))} /><Button variant="outline" size="sm" disabled={action.isPending || view.attempts.some((a) => a.identity.memberId === m.id && a.status === "RUNNING")} onClick={() => run({ action: "replace_member", member: { ...m, providers: [(replacement[m.id] ?? m.providers[0]) as typeof m.providers[number]] } })}>替换原席位绑定</Button></div>)}
    </div></details>}
    {editable && view.plan && <details className="rounded-section border border-line p-4"><summary>显式批准验收契约修订</summary><p className="my-3 text-body">原始要求保持不变。此操作记录人工批准与变更理由，保留源码成果，并重新形成候选、覆盖核对、运行全部检查及独立审查。旧证据保留为历史。</p><label className="text-meta">变更理由<Textarea aria-label="验收修订理由" rows={2} value={acceptanceReason} onChange={(e) => setAcceptanceReason(e.target.value)} /></label><label className="text-meta">新验收清单 JSON（完整 criteria 数组）<Textarea aria-label="验收修订 JSON" rows={10} value={acceptanceText} onChange={(e) => setAcceptanceText(e.target.value)} placeholder={JSON.stringify(view.plan.criteria, null, 2)} /></label><details className="my-2"><summary>查看当前验收清单用于比较</summary><pre className="max-h-80 overflow-auto whitespace-pre-wrap text-meta">{JSON.stringify(view.plan.criteria, null, 2)}</pre></details><Button variant="outline" disabled={action.isPending || !acceptanceText.trim() || !acceptanceReason.trim() || view.attempts.some((a) => a.status === "RUNNING")} onClick={() => { try { const criteria = JSON.parse(acceptanceText) as TeamCriterion[]; if (!Array.isArray(criteria)) throw new Error(); void run({ action: "revise_acceptance", expected_revision: view.planRevision, criteria, reason: acceptanceReason }); } catch { setProblem("验收清单必须为合法 JSON 数组"); } }}>批准验收变更并全部重验</Button></details>}
    {view.plan && <TeamPlanEditor view={view} editable={editable} />}
  </div></div>;
}
