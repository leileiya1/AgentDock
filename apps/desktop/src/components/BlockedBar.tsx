import { useState } from "react";
import { useNavigate } from "react-router-dom";
import type { TaskDetail } from "@/generated/bindings";
import {
  useCancelTask,
  useForceApprove,
  useResumeWithGuidance,
  useRepairReport,
  useApplyRepair,
} from "@/hooks/useTasks";
import { useEvents, useRuns } from "@/hooks/useTaskData";
import { summarizeRunFailure } from "@/lib/execution/runFailure";
import { RunFailureDetail } from "@/components/execution/RunFailureDetail";
import { errorLine } from "@/copy/errors";
import { shortSha } from "@/lib/format";
import { toast } from "@/stores/toastStore";
import { BLOCKED_ACTION_LABEL, blockedCopyFor, latestGateEvent, type BlockedAction } from "@/copy/blocked";
import { Dialog } from "./Dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { BudgetResumeDialog } from "@/components/BudgetResumeDialog";
import { useEnv } from "@/hooks/useEnv";
import { useProviders } from "@/hooks/useProviders";

interface Props {
  task: TaskDetail;
}

const leadCls = "font-semibold text-status-human";

/** BLOCKED 卡片：人话解释为什么停下 + 能做的动作。 */
export function BlockedBar({ task }: Props) {
  const navigate = useNavigate();
  const reasonKey = task.blockedReason;
  const events = useEvents(reasonKey === "quality_gate" ? task.id : undefined);
  const copy = blockedCopyFor(reasonKey, latestGateEvent(events.data));
  // §16: an abnormal exit / unresponsive hang carries structured specifics — surface which Agent
  // failed and how, composed from the already-persisted agent_runs history (no backend change).
  // review_failed is included for consistency; summarizeRunFailure returns null for the non-crash
  // review failures (invalid output, too few council members), so the card only shows on a real crash.
  const showsRunFailure =
    reasonKey === "run_failed" ||
    reasonKey === "agent_unresponsive" ||
    reasonKey === "review_failed";
  const runs = useRuns(showsRunFailure ? task.id : undefined);
  const failure = showsRunFailure
    ? summarizeRunFailure(runs.data ?? [], task.currentRevision)
    : null;
  const resume = useResumeWithGuidance();
  const forceApprove = useForceApprove();
  const cancel = useCancelTask();
  const applyRepair = useApplyRepair();
  const env = useEnv();
  const providers = useProviders();

  const [guidanceOpen, setGuidanceOpen] = useState(false);
  const [asAnswer, setAsAnswer] = useState(false);
  const [text, setText] = useState("");
  const [repairOpen, setRepairOpen] = useState(false);
  const [budgetOpen, setBudgetOpen] = useState(false);
  const repair = useRepairReport(task.id, repairOpen);

  const run = async (fn: () => Promise<unknown>) => {
    try {
      await fn();
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  const openGuidance = (answer: boolean) => {
    setAsAnswer(answer);
    setText("");
    setGuidanceOpen(true);
  };

  const submitGuidance = async () => {
    if (!text.trim()) return;
    await run(() => resume.mutateAsync({ taskId: task.id, guidance: text.trim() }));
    setGuidanceOpen(false);
    setText("");
  };

  const redetect = async () => {
    const [, providerResult] = await Promise.all([env.refetch(), providers.refetch()]);
    const ready = providerResult.data?.filter((provider) => provider.available).length ?? 0;
    toast.info(`状态已刷新：${ready} 个 Provider 可用`);
  };

  const retryAvailable = async () => {
    await redetect();
    await run(() => resume.mutateAsync({
      taskId: task.id,
      guidance: "环境已重新检测；请从保存的检查点继续，并由 AgentFlow 选择降级链中第一个可用 Provider。",
    }));
  };

  const renderAction = (action: BlockedAction) => {
    const label = BLOCKED_ACTION_LABEL[action];
    switch (action) {
      case "answer":
        return <Button key={action} variant="human" onClick={() => openGuidance(true)}>{label}</Button>;
      case "guidance":
        return <Button key={action} variant="human" onClick={() => openGuidance(false)}>{label}</Button>;
      case "forceApprove":
        return (
          <Button key={action} variant="outline" disabled={forceApprove.isPending} onClick={() => run(() => forceApprove.mutateAsync(task.id))}>
            {label}
          </Button>
        );
      case "cancel":
        return (
          <Button key={action} variant="danger" disabled={cancel.isPending} onClick={() => run(() => cancel.mutateAsync(task.id))}>
            {label}
          </Button>
        );
      case "repair":
        return <Button key={action} variant="human" onClick={() => setRepairOpen(true)}>{label}</Button>;
      case "budget":
        return <Button key={action} variant="human" onClick={() => setBudgetOpen(true)}>{label}</Button>;
      case "providerSetup":
        return <Button key={action} variant="human" onClick={() => navigate("/settings#providers")}>{label}</Button>;
      case "redetect":
        return <Button key={action} variant="outline" disabled={env.isFetching || providers.isFetching} onClick={() => run(redetect)}>{env.isFetching || providers.isFetching ? "刷新中…" : label}</Button>;
      case "retryAvailable":
        return <Button key={action} variant="human" disabled={resume.isPending || env.isFetching || providers.isFetching} onClick={() => run(retryAvailable)}>{label}</Button>;
      case "editFallback":
        return <Button key={action} variant="outline" onClick={() => navigate("/settings#project-settings")}>{label}</Button>;
      case "councilSetup":
        return <Button key={action} variant="human" onClick={() => navigate("/settings#project-settings")}>{label}</Button>;
      case "reReview":
        return (
          <Button key={action} variant="outline" disabled={resume.isPending} onClick={() => run(() => resume.mutateAsync({ taskId: task.id, guidance: "已开启审查委员会，请重新审查当前修订。" }))}>
            {label}
          </Button>
        );
    }
  };

  return (
    <>
      <div className="flex flex-col gap-5">
        <div className="flex min-w-0 flex-col gap-1">
          <span className={leadCls}>{copy?.title ?? "需要你处理"}</span>
          <span className="text-body text-t2">{copy?.explanation ?? task.blockedDetail ?? ""}</span>
          {copy?.detailIsQuestion && task.blockedDetail && (
            <blockquote className="mt-1 rounded-r-control border-l-2 border-status-human bg-app/60 px-3 py-2 text-body text-t1">
              {task.blockedDetail}
            </blockquote>
          )}
        </div>
        {failure && <RunFailureDetail failure={failure} />}
        <div className="flex flex-wrap justify-end gap-2">{(copy?.actions ?? ["guidance", "cancel"]).map(renderAction)}</div>
      </div>

      <Dialog
        open={guidanceOpen}
        onClose={() => setGuidanceOpen(false)}
        title={asAnswer ? "回答开发 Agent 的问题" : "补充说明后继续"}
        onConfirmKey={submitGuidance}
        footer={
          <>
            <Button variant="outline" onClick={() => setGuidanceOpen(false)}>取消</Button>
            <Button variant="human" onClick={submitGuidance} disabled={!text.trim() || resume.isPending}>
              {resume.isPending ? "提交中…" : "继续"}
            </Button>
          </>
        }
      >
        {asAnswer && task.blockedDetail && (
          <blockquote className="mb-3 rounded-r-control border-l-2 border-status-human bg-app/60 px-3 py-2 text-body text-t1">
            {task.blockedDetail}
          </blockquote>
        )}
        <div className="flex flex-col gap-2">
          <Label htmlFor="guidance">{asAnswer ? "你的回答" : "补充说明"}</Label>
          <Textarea
            id="guidance"
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder={asAnswer ? "回答会原文交给开发 Agent…" : "补充需求、约束或纠正…"}
            autoFocus
          />
        </div>
      </Dialog>

      <Dialog
        open={repairOpen}
        onClose={() => setRepairOpen(false)}
        title="任务修复中心"
        footer={<Button variant="outline" onClick={() => setRepairOpen(false)}>关闭</Button>}
      >
        {repair.isLoading ? (
          <p className="text-body text-t2">正在检查工作树和检查点…</p>
        ) : repair.isError ? (
          <p className="text-body text-bad">{errorLine(repair.error)}</p>
        ) : repair.data ? (
          <div className="flex flex-col gap-3 text-body">
            <p className="text-t2">
              工作树：{repair.data.worktreeExists ? "存在" : "缺失"} · 残留改动：{repair.data.residualChanges ? "有" : "无"}
            </p>
            {repair.data.latestCheckpoint && (
              <p className="rounded-control bg-app px-3 py-2 text-t2">
                最近检查点 r{repair.data.latestCheckpoint.revision} · {repair.data.latestCheckpoint.phase} · {shortSha(repair.data.latestCheckpoint.commitSha)}
              </p>
            )}
            <div className="flex flex-wrap gap-2">
              {repair.data.actions.map((action) => (
                <Button
                  key={action}
                  variant={action === "reset_to_checkpoint" ? "danger" : "human"}
                  disabled={applyRepair.isPending}
                  onClick={() => run(async () => {
                    await applyRepair.mutateAsync({ taskId: task.id, action });
                    setRepairOpen(false);
                  })}
                >
                  {action === "rebuild_worktree" ? "重建工作树并继续" : action === "resume_residual" ? "保留残留改动继续" : "保存残留后重置"}
                </Button>
              ))}
              {repair.data.actions.length === 0 && <span className="text-t3">当前没有可安全执行的自动修复。</span>}
            </div>
          </div>
        ) : null}
      </Dialog>

      <BudgetResumeDialog task={task} open={budgetOpen} onClose={() => setBudgetOpen(false)} />
    </>
  );
}
