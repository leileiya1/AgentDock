import { useEffect, useMemo, useState } from "react";
import { GitBranch, ShieldAlert, ShieldCheck } from "lucide-react";
import type { AgentKind, Project, ProjectSettings } from "@/generated/bindings";
import { useProjectGitCompatibility, useProjectSettings, usePruneStaleWorktrees, useUpdateProjectSettings } from "@/hooks/useSettings";
import { useProviders } from "@/hooks/useProviders";
import { AGENT_META, ALL_AGENTS, agentLabel } from "@/copy/agents";
import { Toggle } from "@/routes/Settings";
import { SkeletonRows } from "@/components/Skeleton";
import { ErrorState } from "@/components/ErrorState";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { ApiRuntimeSettings } from "@/routes/settings/ApiRuntimeSettings";
import { ProjectConfigTrustPanel } from "@/routes/settings/ProjectConfigTrustPanel";
import { ExecutionPermissionSection } from "@/routes/settings/ExecutionPermissionSection";
import { PermissionRulesPanel } from "@/routes/settings/PermissionRulesPanel";

const FALLBACK_DEVELOPERS = ALL_AGENTS.filter((a) => AGENT_META[a].cli);
const FALLBACK_REVIEWERS = ALL_AGENTS;
const FALLBACK_READ_ONLY = ALL_AGENTS.filter((a) => !AGENT_META[a].cli);

export function SettingsProjectSection({ projects }: { projects: Project[] }) {
  const [projectId, setProjectId] = useState(projects[0].id);
  const settings = useProjectSettings(projectId);
  const gitCompatibility = useProjectGitCompatibility(projectId);
  const update = useUpdateProjectSettings(projectId);
  const pruneWorktrees = usePruneStaleWorktrees(projectId);
  const providers = useProviders();
  const developerAgents = providers.data
    ?.filter((provider) => provider.capabilities.development)
    .map((provider) => provider.id) ?? FALLBACK_DEVELOPERS;
  const reviewerAgents = providers.data
    ?.filter((provider) => provider.capabilities.review)
    .map((provider) => provider.id) ?? FALLBACK_REVIEWERS;
  const readOnlyAgents = providers.data
    ?.filter((provider) => provider.capabilities.review && !provider.capabilities.development)
    .map((provider) => provider.id) ?? FALLBACK_READ_ONLY;

  const [draft, setDraft] = useState<ProjectSettings | null>(null);

  useEffect(() => {
    if (settings.data) setDraft(settings.data);
  }, [settings.data]);

  const patch = (p: Partial<ProjectSettings>) => setDraft((d) => (d ? { ...d, ...p } : d));

  const toggleFallback = (key: "developerFallbacks" | "reviewerFallbacks", agent: AgentKind) => {
    setDraft((d) => {
      if (!d) return d;
      const cur = d[key] ?? [];
      const next = cur.includes(agent) ? cur.filter((a) => a !== agent) : [...cur, agent];
      return { ...d, [key]: next };
    });
  };

  const toggleCouncilReviewer = (agent: AgentKind) => {
    setDraft((d) => {
      if (!d) return d;
      const current = d.reviewCouncil?.reviewers ?? [];
      const reviewers = current.includes(agent)
        ? current.filter((item) => item !== agent)
        : [...current, agent].slice(0, 3);
      return { ...d, reviewCouncil: { ...d.reviewCouncil, enabled: d.reviewCouncil?.enabled ?? false, minimumSuccessfulReviews: d.reviewCouncil?.minimumSuccessfulReviews ?? 2, requireUnanimousPass: d.reviewCouncil?.requireUnanimousPass ?? false, reviewers } };
    });
  };

  const save = async () => {
    if (!draft) return;
    try {
      await update.mutateAsync(draft);
      toast.info("项目设置已保存");
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  const denylistText = useMemo(() => (draft?.envDenylist ?? []).join("\n"), [draft?.envDenylist]);
  const council = {
    enabled: draft?.reviewCouncil?.enabled ?? false,
    reviewers: draft?.reviewCouncil?.reviewers ?? [],
    minimumSuccessfulReviews: draft?.reviewCouncil?.minimumSuccessfulReviews ?? 2,
    requireUnanimousPass: draft?.reviewCouncil?.requireUnanimousPass ?? false,
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-2">
        <Label>选择项目</Label>
        <Select value={projectId} onValueChange={setProjectId}>
          <SelectTrigger><SelectValue /></SelectTrigger>
          <SelectContent>
            {projects.map((p) => <SelectItem key={p.id} value={p.id}>{p.name}</SelectItem>)}
          </SelectContent>
        </Select>
      </div>

      {settings.isLoading || !draft ? (
        <SkeletonRows rows={4} />
      ) : settings.isError ? (
        <ErrorState error={settings.error} onRetry={() => settings.refetch()} compact />
      ) : (
        <>
          <ProjectConfigTrustPanel projectId={projectId} />
          {gitCompatibility.isError ? (
            <ErrorState error={gitCompatibility.error} onRetry={() => gitCompatibility.refetch()} compact />
          ) : gitCompatibility.data ? (
            <div className={`rounded-[var(--radius-panel)] border p-3 ${gitCompatibility.data.blockers.length ? "border-danger/50 bg-danger/5" : "border-line bg-app"}`}>
              <div className="flex items-center gap-2 font-semibold">
                <GitBranch className="size-4" /> Git 兼容性预检
                {gitCompatibility.data.blockers.length ? <ShieldAlert className="size-4 text-danger" /> : <ShieldCheck className="size-4 text-ok" />}
              </div>
              <div className="mt-2 flex flex-wrap gap-1.5 text-[11px] text-t3">
                {gitCompatibility.data.shallow && <span className="rounded bg-raised px-2 py-1">shallow</span>}
                {gitCompatibility.data.sparseCheckout && <span className="rounded bg-raised px-2 py-1">sparse</span>}
                {!!gitCompatibility.data.submodules.length && <span className="rounded bg-raised px-2 py-1">submodule × {gitCompatibility.data.submodules.length}</span>}
                {gitCompatibility.data.lfsTracked && <span className="rounded bg-raised px-2 py-1">Git LFS</span>}
                {gitCompatibility.data.networkFilesystem && <span className="rounded bg-raised px-2 py-1">网络磁盘</span>}
                {gitCompatibility.data.sshRemote && <span className="rounded bg-raised px-2 py-1">SSH remote</span>}
                {!gitCompatibility.data.shallow && !gitCompatibility.data.sparseCheckout && !gitCompatibility.data.submodules.length && !gitCompatibility.data.lfsTracked && <span>标准本地仓库</span>}
              </div>
              <div className="mt-2 grid gap-1 text-[12px] sm:grid-cols-3">
                <PermissionState label="项目可读" ready={gitCompatibility.data.repoReadable} />
                <PermissionState label="项目可写" ready={gitCompatibility.data.repoWritable} />
                <PermissionState label="隔离工作区可写" ready={gitCompatibility.data.worktreeRootWritable} />
              </div>
              {gitCompatibility.data.blockers.map((message) => <p key={message} className="mt-2 text-[12px] text-danger">阻断：{message}</p>)}
              {gitCompatibility.data.warnings.map((message) => <p key={message} className="mt-1 text-[12px] text-human">{message}</p>)}
              {gitCompatibility.data.prunableWorktrees.length > 0 && (
                <div className="mt-3 rounded-md border border-human/40 bg-human-bg px-3 py-2 text-[12px]">
                  <div className="font-medium text-human">失效 worktree 注册 × {gitCompatibility.data.prunableWorktrees.length}</div>
                  <ul className="mt-1 list-inside list-disc break-all text-t3">
                    {gitCompatibility.data.prunableWorktrees.map((path) => <li key={path}>{path}</li>)}
                  </ul>
                  <p className="mt-2 text-t2">清理只执行 Git 的 prune，移除失效注册；不会删除项目源文件或仍存在的工作目录。</p>
                  <Button
                    className="mt-2"
                    variant="outline"
                    size="sm"
                    disabled={pruneWorktrees.isPending}
                    onClick={async () => {
                      try {
                        await pruneWorktrees.mutateAsync();
                        toast.info("失效 worktree 注册已清理，项目文件未被删除");
                      } catch (error) {
                        toast.error(errorLine(error));
                      }
                    }}
                  >
                    {pruneWorktrees.isPending ? "清理中…" : "安全清理失效注册"}
                  </Button>
                </div>
              )}
            </div>
          ) : null}

          <FallbackPicker
            title="开发 Agent 降级顺序"
            hint="首选失败（额度/限流/报错）时按勾选顺序降级。这里只显示支持开发能力的 Provider。"
            options={developerAgents}
            selected={draft.developerFallbacks ?? []}
            onToggle={(a) => toggleFallback("developerFallbacks", a)}
          />

          <div className="rounded-[var(--radius-panel)] border border-line bg-app p-3">
            <Toggle
              label="启用多 Agent 审查委员会"
              checked={council.enabled}
              onChange={(enabled) => patch({ reviewCouncil: { ...council, enabled } })}
            />
            {council.enabled && (
              <div className="mt-3 flex flex-col gap-3 border-t border-line pt-3">
                <FallbackPicker
                  title="委员会候选（最多 3 个厂商）"
                  hint="运行时会自动排除实际开发者和同厂商重复入口；每位成员独立读取同一份审查输入。"
                  options={reviewerAgents}
                  selected={council.reviewers}
                  onToggle={toggleCouncilReviewer}
                />
                <div className="grid grid-cols-2 gap-3">
                  <div className="flex flex-col gap-2">
                    <Label>最少成功成员</Label>
                    <Select
                      value={String(council.minimumSuccessfulReviews)}
                      onValueChange={(value) => patch({ reviewCouncil: { ...council, minimumSuccessfulReviews: Number(value) } })}
                    >
                      <SelectTrigger><SelectValue /></SelectTrigger>
                      <SelectContent><SelectItem value="2">2 人</SelectItem><SelectItem value="3">3 人</SelectItem></SelectContent>
                    </Select>
                  </div>
                  <div className="flex items-end pb-1">
                    <Toggle
                      label="必须全票通过"
                      checked={council.requireUnanimousPass}
                      onChange={(requireUnanimousPass) => patch({ reviewCouncil: { ...council, requireUnanimousPass } })}
                    />
                  </div>
                </div>
              </div>
            )}
          </div>
          <FallbackPicker
            title="审查 Agent 降级顺序"
            hint="审查可用 CLI 或只读 API。"
            options={reviewerAgents}
            selected={draft.reviewerFallbacks ?? []}
            onToggle={(a) => toggleFallback("reviewerFallbacks", a)}
          />

          <div className="flex flex-col gap-2">
            <Label>API 降级 Provider</Label>
            <Select
              value={draft.apiFallbackProvider ?? "__none__"}
              onValueChange={(v) => patch({ apiFallbackProvider: v === "__none__" ? null : (v as AgentKind) })}
            >
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="__none__">不使用</SelectItem>
                {readOnlyAgents.map((a) => <SelectItem key={a} value={a}>{agentLabel(a)}</SelectItem>)}
              </SelectContent>
            </Select>
          </div>

          <ApiRuntimeSettings settings={draft} onPatch={patch} />

          <div className="flex flex-col gap-2">
            <Label>环境变量剔除列表</Label>
            <Textarea
              className="font-mono"
              value={denylistText}
              onChange={(e) => patch({ envDenylist: e.target.value.split("\n").map((s) => s.trim()).filter(Boolean) })}
              placeholder={"每行一个变量名，例如\nAWS_SECRET_ACCESS_KEY"}
            />
            <span className="text-[12px] text-t3">这些环境变量不会传给 Agent 运行。</span>
          </div>

          <Toggle label="复用支持该能力的 Provider 会话（默认关闭）" checked={draft.resumeSessions ?? false} onChange={(v) => patch({ resumeSessions: v })} />

          <ExecutionPermissionSection projectId={projectId} />
          <PermissionRulesPanel projectId={projectId} />

          <div className="flex justify-end">
            <Button variant="primary" onClick={save} disabled={update.isPending}>保存项目设置</Button>
          </div>
        </>
      )}
    </div>
  );
}

function PermissionState({ label, ready }: { label: string; ready: boolean }) {
  return <span className={ready ? "text-ok" : "text-bad"}>{ready ? "✓" : "×"} {label}</span>;
}

function FallbackPicker({ title, hint, options, selected, onToggle }: {
  title: string; hint: string; options: AgentKind[]; selected: AgentKind[]; onToggle: (a: AgentKind) => void;
}) {
  return (
    <div className="flex flex-col gap-2">
      <Label>{title}</Label>
      <div className="flex flex-wrap gap-1.5">
        {options.map((a) => {
          const idx = selected.indexOf(a);
          const on = idx >= 0;
          return (
            <button
              key={a}
              type="button"
              onClick={() => onToggle(a)}
              className={cn(
                "flex items-center gap-1 rounded-full border px-2.5 py-0.5 text-[12px] transition-colors",
                on ? "border-run/60 bg-raised text-t1" : "border-line text-t2 hover:text-t1"
              )}
            >
              {on && (
                <span className="grid size-3.5 place-items-center rounded-full bg-run text-[9px] font-bold text-white">
                  {idx + 1}
                </span>
              )}
              {agentLabel(a)}
            </button>
          );
        })}
      </div>
      <span className="text-[12px] text-t3">{hint}</span>
    </div>
  );
}
