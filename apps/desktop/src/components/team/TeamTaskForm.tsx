import { useState } from "react";
import { useNavigate } from "react-router-dom";
import type { AgentKind, TeamConfig, TeamMember, TeamRole } from "@/generated/bindings";
import { useCreateTeam } from "@/hooks/useTeam";
import { commands } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { Button } from "@/components/ui/button";
import { Input, Textarea } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";

export const DEFAULT_TEAM_CONFIG: TeamConfig = {
  teamCapacity: 4, maxConcurrentAttempts: 2, plannerMemberId: "lead",
  members: [
    { id: "lead", name: "主规划 / 审查", roles: ["planner", "reviewer"], providers: ["claude_code"] },
    { id: "dev-a", name: "开发 A", roles: ["developer"], providers: ["codex"] },
    { id: "dev-b", name: "开发 B", roles: ["developer"], providers: ["codex"] },
    { id: "tester", name: "独立测试", roles: ["tester"], providers: ["codex"] },
  ],
  maxAttempts: 40, verificationReserve: 6, maxReworks: 3, maxPlanRevisions: 4,
  attemptTimeoutSecs: 900, writeGlobs: ["src/**"], allowedCommands: [["node"], ["bun"], ["cargo"], ["python3"]],
  requirePlanApproval: true, crossVendorReview: true,
};
const ROLES: Array<[TeamRole, string]> = [["planner", "规划"], ["developer", "开发"], ["tester", "实测"], ["reviewer", "审查"]];
const selectClass = "rounded-control border border-line bg-app px-2 py-2 text-body text-t1";
export function ProviderSelect({ value, onChange, empty = false }: { value: string; onChange: (v: AgentKind | "") => void; empty?: boolean }) {
  return <select aria-label={empty ? "后备 Provider" : "Provider"} className={selectClass} value={value} onChange={(e) => onChange(e.target.value as AgentKind | "")}>
    {empty && <option value="">无后备</option>}<option value="claude_code">Claude Code</option><option value="codex">GPT / Codex</option>
  </select>;
}
export function TeamTaskForm({ projectId, onClose, onBack }: { projectId: string; onClose: () => void; onBack: () => void }) {
  const [title, setTitle] = useState(""); const [goal, setGoal] = useState(""); const [requirements, setRequirements] = useState("");
  const [config, setConfig] = useState<TeamConfig>(() => structuredClone(DEFAULT_TEAM_CONFIG));
  const [scope, setScope] = useState("src/**"); const [commandPrefixes, setCommandPrefixes] = useState("node\nbun\ncargo\npython3");
  const [target, setTarget] = useState(""); const [busy, setBusy] = useState(false); const [problem, setProblem] = useState<string | null>(null);
  const create = useCreateTeam(); const navigate = useNavigate();
  const editMember = (id: string, update: Partial<TeamMember>) => setConfig((c) => ({ ...c, members: c.members.map((m) => m.id === id ? { ...m, ...update } : m) }));
  const number = (key: keyof TeamConfig, value: string) => setConfig((c) => ({ ...c, [key]: Number(value) }));
  const submit = async (start: boolean) => {
    if (busy) return; setBusy(true); setProblem(null);
    try {
      const view = await create.mutateAsync({ projectId, title: title.trim(), goal, requirements: requirements.split("\n").filter((r) => r.trim()), targetBranch: target.trim() || null,
        config: { ...config, writeGlobs: scope.split("\n").map((s) => s.trim()).filter(Boolean), allowedCommands: commandPrefixes.split("\n").filter((s) => s.trim()).map((s) => s.trim().split(/\s+/)) } });
      if (start) await unwrap(commands.teamAction({ taskId: view.taskId, action: { action: "start" } }));
      onClose(); navigate(`/p/${projectId}/t/${view.taskId}`);
    } catch (e) { const message = errorLine(e); setProblem(message); toast.error(message); } finally { setBusy(false); }
  };
  return <div className="flex flex-col gap-4" data-testid="team-create-form">
    <p className="text-body text-t2">本机 Git 团队执行：独立规划、工作项并行、候选集成、逐条实测和独立审查。最终本地交付单独批准。</p>
    <Label htmlFor="team-title">标题</Label><Input id="team-title" value={title} onChange={(e) => setTitle(e.target.value)} />
    <Label htmlFor="team-goal">原始目标（原样保存）</Label><Textarea id="team-goal" value={goal} onChange={(e) => setGoal(e.target.value)} rows={3} />
    <Label htmlFor="team-requirements">必须满足的条件（每行一条）</Label><Textarea id="team-requirements" value={requirements} onChange={(e) => setRequirements(e.target.value)} rows={3} placeholder="包含预期行为、失败场景和实测要求" />
    <div className="grid grid-cols-2 gap-3"><label className="text-meta">团队人数上限<Input aria-label="团队人数上限" type="number" min={3} max={16} value={config.teamCapacity} onChange={(e) => number("teamCapacity", e.target.value)} /></label>
      <label className="text-meta">同时运行上限<Input aria-label="同时运行上限" type="number" min={1} max={config.teamCapacity} value={config.maxConcurrentAttempts} onChange={(e) => number("maxConcurrentAttempts", e.target.value)} /></label></div>
    <p className="text-meta text-t3">人数限制逻辑席位，重试/后备复用原席位。同一 CLI 可绑定多个席位；实际并行还受设置中的 Provider/账号并发限制。不可用候选在运行预检中明确阻断。</p>
    {config.members.map((member) => <div className="flex flex-col gap-2 rounded-control border border-line p-3" key={member.id}>
      <div className="flex flex-wrap gap-2"><Input aria-label={`${member.id} 名称`} className="min-w-24 flex-1" value={member.name} onChange={(e) => editMember(member.id, { name: e.target.value })} />
        <ProviderSelect value={member.providers[0]} onChange={(v) => v && editMember(member.id, { providers: [v, ...member.providers.slice(1)] })} />
        <ProviderSelect empty value={member.providers[1] ?? ""} onChange={(v) => editMember(member.id, { providers: v ? [member.providers[0], v] : [member.providers[0]] })} />
        <Button variant="ghost" size="sm" onClick={() => setConfig((c) => ({ ...c, members: c.members.filter((m) => m.id !== member.id) }))}>移除</Button></div>
      <div className="flex flex-wrap gap-4">{ROLES.map(([role, label]) => <label key={role} className="flex items-center gap-1 text-meta"><input type="checkbox" checked={member.roles.includes(role)} onChange={(e) => editMember(member.id, { roles: e.target.checked ? [...member.roles, role] : member.roles.filter((r) => r !== role) })} />{label}</label>)}</div>
    </div>)}
    <Button variant="outline" disabled={config.members.length >= config.teamCapacity} onClick={() => setConfig((c) => ({ ...c, members: [...c.members, { id: `member-${crypto.randomUUID().slice(0, 8)}`, name: "新成员", roles: ["developer"], providers: ["codex"] }] }))}>添加席位</Button>
    <label className="flex flex-col gap-1 text-meta">独立主规划<select className={selectClass} aria-label="独立主规划" value={config.plannerMemberId} onChange={(e) => setConfig((c) => ({ ...c, plannerMemberId: e.target.value }))}>{config.members.filter((m) => m.roles.includes("planner")).map((m) => <option value={m.id} key={m.id}>{m.name} · {m.providers.join(" → ")}</option>)}</select></label>
    <details><summary className="cursor-pointer text-body">运行范围与恢复预算</summary><div className="mt-3 flex flex-col gap-3">
      <label className="text-meta">允许修改路径（glob，每行一条）<Textarea aria-label="允许修改路径" rows={2} value={scope} onChange={(e) => setScope(e.target.value)} /></label>
      <label className="text-meta">授权命令前缀（每行一个 argv 前缀；不解析 shell）<Textarea aria-label="授权命令前缀" rows={4} value={commandPrefixes} onChange={(e) => setCommandPrefixes(e.target.value)} /></label>
      <label className="text-meta">目标分支（空为项目默认）<Input value={target} onChange={(e) => setTarget(e.target.value)} /></label>
      <div className="grid grid-cols-2 gap-3">{([["maxAttempts", "累计尝试上限"], ["verificationReserve", "验证保留次数"], ["maxReworks", "累计产品返工上限"], ["maxPlanRevisions", "计划版本上限"], ["attemptTimeoutSecs", "单次运行秒数"]] as const).map(([key, label]) => <label key={key} className="text-meta">{label}<Input type="number" min={0} aria-label={label} value={config[key]} onChange={(e) => number(key, e.target.value)} /></label>)}</div>
      <label className="text-body"><input type="checkbox" checked={config.requirePlanApproval} onChange={(e) => setConfig((c) => ({ ...c, requirePlanApproval: e.target.checked }))} /> 确认计划后执行</label>
      <label className="text-body"><input type="checkbox" checked={config.crossVendorReview} onChange={(e) => setConfig((c) => ({ ...c, crossVendorReview: e.target.checked }))} /> 跨厂商独立审查（不足时阻断）</label>
      <p className="text-meta text-t3">预算约束受管尝试次数与时长。费用、Token 未知时保持未知；本轮不提供硬费用保证。敏感变更仍要求多厂商审查委员会；仅两个厂商时可能无法满足。</p>
    </div></details>
    {problem && <p role="alert" className="text-body text-status-failed">{problem}</p>}
    <div className="flex justify-end gap-2"><Button variant="ghost" onClick={onBack}>返回旧流程</Button><Button variant="outline" disabled={busy || !title.trim() || !goal.trim()} onClick={() => submit(false)}>保存草稿</Button><Button variant="primary" disabled={busy || !title.trim() || !goal.trim()} onClick={() => submit(true)}>{busy ? "创建中…" : "创建并启动团队"}</Button></div>
  </div>;
}
