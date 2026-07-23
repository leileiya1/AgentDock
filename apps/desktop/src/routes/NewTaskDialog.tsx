import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { AlertTriangle, Plus, Trash2 } from "lucide-react";
import type {
  AcceptanceCriterionInput,
  AcceptanceCriterionKind,
  AgentKind,
  DeliveryMode,
  ProviderDescriptor,
  ProviderPreflightArgs,
  TaskPreflightReport,
} from "@/generated/bindings";
import { AGENT_META, ALL_AGENTS, isApiAgent } from "@/copy/agents";
import { useProjects } from "@/hooks/useProjects";
import { useProjectGitCompatibility, useProjectSettings } from "@/hooks/useSettings";
import { useProviders } from "@/hooks/useProviders";
import { useProviderPreflight } from "@/hooks/usePreflight";
import { useCreateTask, useStartTask } from "@/hooks/useTasks";
import { useExecutionNodes } from "@/hooks/useGovernance";
import { useUiStore } from "@/stores/uiStore";
import { errorLine } from "@/copy/errors";
import { preflightBlockLine, summarizePreflight } from "@/lib/preflight";
import { toast } from "@/stores/toastStore";
import { Dialog } from "@/components/Dialog";
import { PreflightBlockedDialog } from "@/components/PreflightBlockedDialog";
import { Button } from "@/components/ui/button";
import { Input, Textarea } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { maxRevisionsError, parseMaxRevisions } from "@/lib/taskForm";

type AcceptanceDraft = AcceptanceCriterionInput & { id: string };

const ACCEPTANCE_KIND_LABEL: Record<AcceptanceCriterionKind, string> = {
  build: "构建",
  test: "测试",
  behavior: "行为",
  manual: "人工验收",
};

const FALLBACK_PROVIDERS: ProviderDescriptor[] = ALL_AGENTS.map((id) => ({
  id,
  displayName: AGENT_META[id].label,
  source: "builtin",
  protocolVersion: "1.0",
  capabilities: {
    development: AGENT_META[id].cli,
    review: true,
    streaming: true,
    structuredOutput: true,
    sandbox: true,
    resume: AGENT_META[id].cli,
  },
  executionLocation: isApiAgent(id) ? "remote" : "local",
  dataEgress: isApiAgent(id) ? "diff" : "none",
  permissions: {
    worktreeRead: !isApiAgent(id),
    worktreeWrite: AGENT_META[id].cli,
    networkDomains: isApiAgent(id) ? ["provider-api"] : [],
    commands: [],
  },
  trust: "builtin",
  available: true,
  problem: null,
}));

export function NewTaskDialog() {
  const projectId = useUiStore((s) => s.newTaskProjectId);
  const close = useUiStore((s) => s.closeNewTask);
  const projects = useProjects();
  const projectSettings = useProjectSettings(projectId ?? undefined);
  const gitCompatibility = useProjectGitCompatibility(projectId ?? undefined);
  const providers = useProviders();
  const nodes = useExecutionNodes();
  const create = useCreateTask();
  const start = useStartTask();
  const preflight = useProviderPreflight();
  const navigate = useNavigate();

  const project = useMemo(() => projects.data?.find((p) => p.id === projectId), [projects.data, projectId]);
  const catalog = providers.data ?? FALLBACK_PROVIDERS;
  const developerProviders = catalog.filter((provider) => provider.capabilities.development);
  const reviewerProviders = catalog.filter((provider) => provider.capabilities.review);

  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [acceptanceCriteria, setAcceptanceCriteria] = useState<AcceptanceDraft[]>([]);
  const [developerAgent, setDeveloperAgent] = useState<AgentKind>("claude_code");
  const [reviewerAgent, setReviewerAgent] = useState<AgentKind>("codex");
  const [targetBranch, setTargetBranch] = useState("");
  const [maxRevisions, setMaxRevisions] = useState("3");
  const [maxRevisionsTouched, setMaxRevisionsTouched] = useState(false);
  const [allowApiEgress, setAllowApiEgress] = useState(false);
  const [requirePlanApproval, setRequirePlanApproval] = useState(true);
  const [tokenBudget, setTokenBudget] = useState(500_000);
  const [costBudgetUsd, setCostBudgetUsd] = useState(25);
  const [timeBudgetSecs, setTimeBudgetSecs] = useState(7_200);
  const [minimumQualityScore, setMinimumQualityScore] = useState(70);
  const [priority, setPriority] = useState(0);
  const [deliveryMode, setDeliveryMode] = useState<DeliveryMode>("local_merge");
  const [executionNodeId, setExecutionNodeId] = useState("local");
  const [preflightReport, setPreflightReport] = useState<TaskPreflightReport | null>(null);

  const open = !!projectId;
  const sameAgent = developerAgent === reviewerAgent;
  const descriptor = (id: AgentKind) => catalog.find((provider) => provider.id === id);
  const requiresEgress = (id: AgentKind) => {
    const provider = descriptor(id);
    return isApiAgent(id)
      || provider?.executionLocation !== "local"
      || provider?.dataEgress !== "none"
      || (provider?.permissions.networkDomains?.length ?? 0) > 0;
  };
  const directEgress = requiresEgress(developerAgent) || requiresEgress(reviewerAgent);
  const fallbackMayUseApi = projectSettings.data?.reviewerFallbacks?.some(requiresEgress) ?? false;
  const councilMayUseApi = projectSettings.data?.reviewCouncil?.enabled
    ? projectSettings.data.reviewCouncil.reviewers?.some(requiresEgress) ?? false
    : false;
  const apiMayBeUsed = directEgress || fallbackMayUseApi || councilMayUseApi;
  const maxRevisionsProblem = maxRevisionsError(maxRevisions);
  const criteriaValid = acceptanceCriteria.every((criterion) => criterion.text.trim().length > 0 && criterion.text.trim().length <= 500);
  const canSubmit = !!title.trim()
    && !sameAgent
    && (!directEgress && !councilMayUseApi || allowApiEgress)
    && !maxRevisionsProblem
    && criteriaValid
    && !create.isPending
    && !start.isPending
    && !preflight.isPending;

  const reset = () => {
    setTitle("");
    setDescription("");
    setAcceptanceCriteria([]);
    setDeveloperAgent("claude_code");
    setReviewerAgent("codex");
    setTargetBranch("");
    setMaxRevisions("3");
    setMaxRevisionsTouched(false);
    setAllowApiEgress(false);
    setRequirePlanApproval(true);
    setTokenBudget(500_000);
    setCostBudgetUsd(25);
    setTimeBudgetSecs(7_200);
    setMinimumQualityScore(70);
    setPriority(0);
    setDeliveryMode("local_merge");
    setExecutionNodeId("local");
    setPreflightReport(null);
  };
  const onClose = () => {
    close();
    reset();
  };

  const preflightArgs = (): ProviderPreflightArgs => ({
    projectId: projectId!,
    developerAgent,
    reviewerAgent,
    allowApiEgress,
    requirePlanApproval,
  });

  // The happy path: create the task and, if requested, start it, then navigate to its detail.
  const createThenStart = async (thenStart: boolean) => {
    const detail = await create.mutateAsync({
      projectId: projectId!,
      title: title.trim(),
      description: description.trim(),
      acceptanceCriteria: acceptanceCriteria.map(({ kind, text }) => ({ kind, text: text.trim() })),
      developerAgent,
      reviewerAgent,
      targetBranch: targetBranch.trim() || null,
      maxRevisions: parseMaxRevisions(maxRevisions),
      allowApiEgress,
      policy: {
        requirePlanApproval,
        priority,
        tokenBudget,
        costBudgetUsd,
        timeBudgetSecs,
        minimumQualityScore,
        deliveryMode,
        executionNodeId: executionNodeId === "local" ? null : executionNodeId,
      },
    });
    if (thenStart) {
      try {
        await start.mutateAsync(detail.id);
      } catch (e) {
        toast.error(errorLine(e));
      }
    }
    onClose();
    navigate(`/p/${projectId}/t/${detail.id}`);
  };

  const submit = async (thenStart: boolean) => {
    setMaxRevisionsTouched(true);
    if (!projectId || !canSubmit) return;
    try {
      // Before spawning a run, verify at least one developer and one reviewer Provider can really
      // run. If not, list every reason instead of creating a task that would only fail (P0-01/02).
      if (thenStart) {
        const report = await preflight.mutateAsync(preflightArgs());
        if (!report.ready) {
          setPreflightReport(report);
          return;
        }
      }
      await createThenStart(thenStart);
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  // Re-run the live probe from the block dialog; if the environment is now fixed, continue straight
  // into create-and-start so the user does not have to re-open anything.
  const redetect = async () => {
    if (!projectId) return;
    try {
      const report = await preflight.mutateAsync(preflightArgs());
      if (report.ready) {
        setPreflightReport(null);
        await createThenStart(true);
      } else {
        setPreflightReport(report);
        toast.info(preflightBlockLine(summarizePreflight(report)));
      }
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  if (!open) return null;

  return (
    <>
    <Dialog
      open={open}
      onClose={onClose}
      title="新建任务"
      width={560}
      onConfirmKey={() => submit(false)}
      footer={
        <>
          <Button variant="outline" onClick={onClose}>取消</Button>
          <Button variant="subtle" disabled={!canSubmit} onClick={() => submit(true)}>
            {preflight.isPending ? "检测环境…" : "创建并立即开始"}
          </Button>
          <Button variant="primary" disabled={!canSubmit} onClick={() => submit(false)}>
            {create.isPending ? "创建中…" : "创建"}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        {(gitCompatibility.data?.prunableWorktrees.length ?? 0) > 0 && (
          <div className="flex items-start gap-2 rounded-md border border-human/50 bg-human-bg px-3 py-2 text-[13px]">
            <AlertTriangle className="mt-0.5 size-4 shrink-0 text-human" />
            <div className="min-w-0 flex-1">
              <div className="font-medium text-human">检测到 {gitCompatibility.data?.prunableWorktrees.length} 个失效 worktree 注册</div>
              <p className="mt-0.5 text-t2">不阻断创建，但建议先安全清理，避免 Git 工作树状态持续积累。</p>
            </div>
            <Button variant="outline" size="sm" onClick={() => { close(); navigate("/settings#project-settings"); }}>
              查看并清理
            </Button>
          </div>
        )}
        <div className="flex flex-col gap-2">
          <Label htmlFor="nt-title">标题</Label>
          <Input id="nt-title" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="一句话说清要做什么" autoFocus />
        </div>

        <div className="flex flex-col gap-2 rounded-md border border-line bg-app/50 p-3">
          <div className="flex items-start justify-between gap-3">
            <div>
              <Label>结构化验收条件（可选）</Label>
              <p className="mt-1 text-[12px] text-t3">构建、测试、行为和人工要求会独立保存，并在验证、审查和最终批准时逐条回显。</p>
            </div>
            <Button
              variant="outline"
              size="sm"
              disabled={acceptanceCriteria.length >= 20}
              onClick={() => setAcceptanceCriteria((current) => [
                ...current,
                { id: crypto.randomUUID(), kind: "behavior", text: "" },
              ])}
            >
              <Plus className="size-3.5" /> 添加
            </Button>
          </div>
          {acceptanceCriteria.length === 0 ? (
            <div className="rounded-md border border-dashed border-line px-3 py-2 text-[12px] text-t3">还没有独立验收条件；描述仍会原样交给 Agent。</div>
          ) : (
            <div className="flex flex-col gap-2">
              {acceptanceCriteria.map((criterion, index) => (
                <div key={criterion.id} className="grid grid-cols-[7.5rem_minmax(0,1fr)_auto] items-center gap-2">
                  <Select
                    value={criterion.kind}
                    onValueChange={(value) => setAcceptanceCriteria((current) => current.map((item) => (
                      item.id === criterion.id ? { ...item, kind: value as AcceptanceCriterionKind } : item
                    )))}
                  >
                    <SelectTrigger aria-label={`验收条件 ${index + 1} 类型`}><SelectValue /></SelectTrigger>
                    <SelectContent>
                      {(Object.entries(ACCEPTANCE_KIND_LABEL) as Array<[AcceptanceCriterionKind, string]>).map(([value, label]) => (
                        <SelectItem key={value} value={value}>{label}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Input
                    aria-label={`验收条件 ${index + 1}`}
                    value={criterion.text}
                    maxLength={500}
                    onChange={(event) => setAcceptanceCriteria((current) => current.map((item) => (
                      item.id === criterion.id ? { ...item, text: event.target.value } : item
                    )))}
                    placeholder="写清可判断的完成标准"
                  />
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={`删除验收条件 ${index + 1}`}
                    onClick={() => setAcceptanceCriteria((current) => current.filter((item) => item.id !== criterion.id))}
                  >
                    <Trash2 className="size-4" />
                  </Button>
                </div>
              ))}
              {!criteriaValid && <p className="text-[12px] text-bad">每条验收条件必须填写 1–500 个字符。</p>}
            </div>
          )}
        </div>

        <div className="flex flex-col gap-2">
          <Label htmlFor="nt-desc">描述</Label>
          <Textarea
            id="nt-desc"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="说明会原文交给开发 Agent，写清楚背景、期望和约束。"
          />
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div className="flex flex-col gap-2">
            <Label>开发 Agent</Label>
            <Select value={developerAgent} onValueChange={(v) => setDeveloperAgent(v as AgentKind)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {developerProviders.map((provider) => (
                  <SelectItem key={provider.id} value={provider.id} disabled={!provider.available}>
                    {provider.displayName}{provider.source === "external" ? "（协议）" : ""}{provider.available ? "" : "（不可用）"}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex flex-col gap-2">
            <Label>审查 Agent</Label>
            <Select value={reviewerAgent} onValueChange={(v) => setReviewerAgent(v as AgentKind)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {reviewerProviders.map((provider) => (
                  <SelectItem key={provider.id} value={provider.id} disabled={!provider.available}>
                    {provider.displayName}{provider.capabilities.development ? "" : "（只读审查）"}{provider.source === "external" ? "（协议）" : ""}{provider.available ? "" : "（不可用）"}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        {sameAgent && (
          <div className="flex items-start gap-2 rounded-md border border-human bg-human-bg px-3 py-2 text-[13px] text-human">
            <AlertTriangle className="mt-0.5 size-4 shrink-0" />
            开发和审查不能用同一个 Agent。同源审查会显著降低缺陷检出——异构互审正是本工具的核心价值。
          </div>
        )}

        {apiMayBeUsed && (
          <div className="flex items-start gap-3 rounded-md border border-line bg-app px-3 py-3 text-[13px]">
            <Switch
              id="nt-api-egress"
              checked={allowApiEgress}
              onCheckedChange={setAllowApiEgress}
              aria-label="允许 API 数据外发"
            />
            <label htmlFor="nt-api-egress" className="cursor-pointer leading-5 text-t2">
              <span className="block font-medium text-t1">允许 API 审查外发</span>
              可能会把任务描述、代码差异和测试摘要发送给已配置的第三方 API。密钥不会进入提示内容或运行日志。
              {!allowApiEgress && !directEgress && !councilMayUseApi && (
                <span className="mt-1 block text-t3">保持关闭时，只会使用本地 CLI 审查和降级链。</span>
              )}
              {!allowApiEgress && (directEgress || councilMayUseApi) && (
                <span className="mt-1 block text-human">
                  {directEgress ? "当前开发或审查 Provider 需要数据外发" : "审查委员会包含需要数据外发的成员"}，确认后才能创建。
                </span>
              )}
            </label>
          </div>
        )}

        {providers.isError && (
          <div className="text-[12px] text-human">Provider 清单读取失败，当前暂用内置清单。</div>
        )}

        <div className="grid grid-cols-2 gap-4">
          <div className="flex flex-col gap-2">
            <Label htmlFor="nt-branch">目标分支</Label>
            <Input id="nt-branch" className="font-mono" value={targetBranch} onChange={(e) => setTargetBranch(e.target.value)} placeholder={project?.defaultBranch ?? "main"} />
            <span className="text-[12px] text-t3">留空则使用项目默认分支 {project?.defaultBranch ?? "main"}。</span>
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="nt-max">最大返工轮数</Label>
            <Input
              id="nt-max"
              type="text"
              inputMode="numeric"
              value={maxRevisions}
              aria-invalid={maxRevisionsTouched && !!maxRevisionsProblem}
              aria-describedby="nt-max-help"
              onChange={(event) => setMaxRevisions(event.target.value)}
              onBlur={() => setMaxRevisionsTouched(true)}
            />
            <span id="nt-max-help" className={`text-[12px] ${maxRevisionsTouched && maxRevisionsProblem ? "text-bad" : "text-t3"}`}>
              {maxRevisionsTouched && maxRevisionsProblem ? maxRevisionsProblem : "允许 1–20 轮；输入完成后校验，不会自动改写。"}
            </span>
          </div>
        </div>

        <div className="rounded-md border border-line bg-app/60 p-3">
          <div className="mb-3 flex items-start justify-between gap-4">
            <div>
              <div className="text-[13px] font-medium text-t1">执行控制</div>
              <div className="mt-0.5 text-[12px] text-t3">计划门禁、硬预算、质量阈值和交付方式会随任务固化。</div>
            </div>
            <label className="flex shrink-0 items-center gap-2 text-[12px] text-t2">
              <Switch checked={requirePlanApproval} onCheckedChange={setRequirePlanApproval} />
              编码前审批计划
            </label>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-1.5">
              <Label>交付方式</Label>
              <Select value={deliveryMode} onValueChange={(value) => setDeliveryMode(value as DeliveryMode)}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="local_merge">本地安全合并</SelectItem>
                  <SelectItem value="github_pr">GitHub PR + CI</SelectItem>
                  <SelectItem value="gitlab_mr">GitLab MR + CI</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="flex flex-col gap-1.5">
              <Label>验证执行位置</Label>
              <Select value={executionNodeId} onValueChange={setExecutionNodeId}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="local">本机</SelectItem>
                  {(nodes.data ?? []).filter((node) => node.enabled).map((node) => (
                    <SelectItem key={node.id} value={node.id} disabled={node.status === "offline"}>
                      {node.name}{node.status === "online" ? " · 在线" : node.status === "offline" ? " · 不可用" : " · 未检查"}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex flex-col gap-1.5">
              <Label>队列优先级</Label>
              <Select value={String(priority)} onValueChange={(value) => setPriority(Number(value))}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="50">高（优先调度）</SelectItem>
                  <SelectItem value="0">普通</SelectItem>
                  <SelectItem value="-20">后台</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <div className="mt-3 grid grid-cols-4 gap-2">
            <BudgetField label="Token" value={tokenBudget} onChange={setTokenBudget} />
            <BudgetField label="费用 ($)" value={costBudgetUsd} onChange={setCostBudgetUsd} step="0.5" />
            <BudgetField label="时间 (秒)" value={timeBudgetSecs} onChange={setTimeBudgetSecs} />
            <BudgetField label="最低质量" value={minimumQualityScore} onChange={setMinimumQualityScore} max={100} />
          </div>
        </div>
      </div>
    </Dialog>
    <PreflightBlockedDialog
      open={!!preflightReport}
      report={preflightReport}
      redetecting={preflight.isPending}
      onRedetect={redetect}
      onClose={() => setPreflightReport(null)}
    />
    </>
  );
}

function BudgetField({ label, value, onChange, step, max }: {
  label: string;
  value: number;
  onChange: (value: number) => void;
  step?: string;
  max?: number;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label>{label}</Label>
      <Input type="number" min={1} max={max} step={step} value={value} onChange={(event) => onChange(Math.max(1, Number(event.target.value) || 1))} />
    </div>
  );
}
