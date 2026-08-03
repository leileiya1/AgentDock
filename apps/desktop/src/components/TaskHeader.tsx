import { useState } from "react";
import { Link } from "react-router-dom";
import { ArrowRight, ChevronLeft, Info, Laptop, Server } from "lucide-react";
import type { TaskDetail } from "@/generated/bindings";
import { agentLabel } from "@/copy/agents";
import { budgetView } from "@/lib/governance/budget";
import { taskCode, shortSha, absoluteTime } from "@/lib/format";
import { AgentMark } from "@/components/AgentMark";
import { CopyText } from "@/components/CopyText";
import { Dialog } from "@/components/Dialog";
import { StateBadge } from "@/components/StateBadge";
import { QueueControls } from "@/components/QueueControls";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const DELIVERY_LABEL: Record<string, string> = {
  local_merge: "本地安全合并",
  github_pr: "GitHub PR",
  gitlab_mr: "GitLab MR",
};

const DELIVERY_STATE_LABEL: Record<string, string> = {
  pending: "准备中",
  open: "已创建，等待处理",
  ci_running: "CI 运行中",
  ready: "检查通过，可合并",
  merged: "已合并",
  failed: "未通过",
  rolled_back: "已回滚",
};

/**
 * 顶部任务头 (05 §5.2). 第一行是身份和总状态；第二行是**可读摘要**——
 * Agent 分工、当前轮、验证位置、预算健康、交付状态。
 * 分支、base SHA 这类次要技术信息移进「任务信息」，不再占据头部一整排灰字。
 */
export function TaskHeader({
  task,
  projectId,
  phaseSummary,
}: {
  task: TaskDetail;
  projectId: string | undefined;
  /** 例：`审查中 · 2/3 成员完成`，由执行树推导，保证与左侧一致。 */
  phaseSummary: string | null;
}) {
  const [infoOpen, setInfoOpen] = useState(false);
  const budget = budgetView(task.budget);
  const remote = task.policy.executionNodeId != null;

  return (
    <header className="flex shrink-0 flex-col gap-2 border-b border-line/70 px-6 py-3">
      <div className="flex min-w-0 items-center gap-3">
        <Link
          to={`/p/${projectId}`}
          className="flex h-7 items-center gap-1 rounded-control px-2 text-body text-t3 transition-colors hover:bg-raised hover:text-t1"
          title="返回任务列表"
        >
          <ChevronLeft className="size-4" /> 返回
        </Link>
        <span className="shrink-0 font-mono text-body text-t3">{taskCode(task.seq)}</span>
        <h1 className="min-w-0 flex-1 truncate text-section font-semibold">{task.title}</h1>
        <StateBadge status={task.status} />
      </div>

      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-meta">
        {/* 谁在做什么——图标 30 px (05 §5.4)。 */}
        <span className="flex items-center gap-1.5" title="开发与审查的 Agent 分工">
          <AgentMark kind={task.developerAgent} size={30} />
          <span className="text-t2">开发 {agentLabel(task.developerAgent)}</span>
          <ArrowRight className="size-3.5 text-t3" aria-hidden />
          <AgentMark kind={task.reviewerAgent} size={30} />
          <span className="text-t2">审查 {agentLabel(task.reviewerAgent)}</span>
        </span>

        <Divider />
        <span className="text-t2">
          当前 r{task.currentRevision}
          {phaseSummary && ` · ${phaseSummary}`}
        </span>

        <Divider />
        <span className="flex items-center gap-1 text-t2" title={remote ? "验证在远程节点执行" : "验证在本机执行"}>
          {remote ? <Server className="size-3.5 text-t3" aria-hidden /> : <Laptop className="size-3.5 text-t3" aria-hidden />}
          {remote ? "远程验证" : "本机验证"}
        </span>

        <Divider />
        <span
          className={cn(
            "text-t2",
            budget.level === "blocked" && "text-bad",
            budget.level === "warn" && "text-caution"
          )}
          title="详细预算在治理页"
        >
          {budget.headline}
        </span>

        {task.delivery && (
          <>
            <Divider />
            <span className="flex items-center gap-1 text-t2">
              交付：{DELIVERY_LABEL[task.delivery.mode] ?? task.delivery.mode}
              {task.delivery.number != null && ` #${task.delivery.number}`}
              <span className="text-t3">
                · {DELIVERY_STATE_LABEL[task.delivery.state] ?? task.delivery.state}
              </span>
            </span>
          </>
        )}

        <span className="ml-auto flex items-center gap-2">
          <QueueControls task={task} />
          <Button variant="ghost" size="sm" onClick={() => setInfoOpen(true)}>
            <Info className="size-3.5" /> 任务信息
          </Button>
        </span>
      </div>

      <Dialog open={infoOpen} onClose={() => setInfoOpen(false)} title="任务信息" width={520}>
        <dl className="grid grid-cols-[92px_1fr] gap-x-4 gap-y-2.5 text-body">
          <InfoRow label="目标分支">{task.targetBranch}</InfoRow>
          {task.branch && <InfoRow label="工作分支">{task.branch}</InfoRow>}
          {task.baseCommit && (
            <InfoRow label="base commit">
              <CopyText value={task.baseCommit} className="font-mono">
                {shortSha(task.baseCommit, 12)}
              </CopyText>
            </InfoRow>
          )}
          <InfoRow label="交付方式">{DELIVERY_LABEL[task.policy.deliveryMode ?? "local_merge"]}</InfoRow>
          <InfoRow label="返工上限">最多 {task.maxRevisions} 轮</InfoRow>
          <InfoRow label="验收条件">{task.acceptanceCriteria.length > 0 ? `${task.acceptanceCriteria.length} 条结构化条件` : "未单独设置"}</InfoRow>
          <InfoRow label="计划审批">{task.policy.requirePlanApproval ? "开启：写代码前需要你批准计划" : "关闭"}</InfoRow>
          <InfoRow label="最近更新">{absoluteTime(task.updatedAt)}</InfoRow>
        </dl>
      </Dialog>
    </header>
  );
}

const Divider = () => <span className="text-t3/50" aria-hidden>·</span>;

function InfoRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <>
      <dt className="text-t3">{label}</dt>
      <dd className="min-w-0 break-words text-t1">{children}</dd>
    </>
  );
}
