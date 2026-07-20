import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { AnimatePresence, motion } from "motion/react";
import { AlertTriangle } from "lucide-react";
import type { TaskDetail } from "@/generated/bindings";
import {
  useApproveTask,
  useCancelTask,
  useForceApprove,
  useMarkMergedExternal,
  useRejectTask,
  useResumeWithGuidance,
  useRepairReport,
  useApplyRepair,
  useStartTask,
} from "@/hooks/useTasks";
import { useDiff, useReview } from "@/hooks/useTaskData";
import { qk } from "@/lib/queryKeys";
import { shortSha } from "@/lib/format";
import { toAppError, errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { BLOCKED_COPY, BLOCKED_ACTION_LABEL, type BlockedAction } from "@/copy/blocked";
import { CopyText } from "./CopyText";
import { Dialog } from "./Dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PlanApprovalBar } from "@/components/PlanApprovalBar";
import { useDeliveryRefresh, useDeliveryStart } from "@/hooks/useGovernance";
import { BudgetResumeDialog } from "@/components/BudgetResumeDialog";

interface Props {
  task: TaskDetail;
}

const ATTENTION = new Set(["DRAFT", "WAITING_FOR_PLAN_APPROVAL", "WAITING_FOR_HUMAN_APPROVAL", "APPROVED", "MERGE_CONFLICT", "BLOCKED"]);

export function ApprovalBar({ task }: Props) {
  const show = ATTENTION.has(task.status);

  // A blocked task needs an explicit user decision, so surface it in the
  // viewport instead of letting the action bar fall below short windows.
  if (task.status === "BLOCKED") {
    return (
      <AnimatePresence>
        {show && (
          <motion.div
            key={task.status}
            role="dialog"
            aria-modal="true"
            aria-label="需要你处理"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.16 }}
            className="fixed inset-0 z-40 grid place-items-center overflow-y-auto bg-black/55 p-5 backdrop-blur-[3px]"
          >
            <motion.div
              initial={{ opacity: 0, y: 12, scale: 0.97 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: 8, scale: 0.98 }}
              transition={{ type: "spring", stiffness: 380, damping: 30 }}
              className="relative flex max-h-[calc(100vh-2.5rem)] w-full max-w-2xl flex-col overflow-y-auto rounded-[var(--radius-panel)] border border-human bg-human-bg/95 p-5 shadow-[var(--shadow-float)]"
            >
              <span className="pointer-events-none absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-human/60 to-transparent" />
              <BlockedBar task={task} />
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    );
  }

  return (
    <AnimatePresence>
      {show && (
        <motion.div
          key={task.status}
          role="region"
          aria-label="需要你处理"
          initial={{ y: "100%", opacity: 0 }}
          animate={{ y: 0, opacity: 1 }}
          exit={{ y: "100%", opacity: 0 }}
          transition={{ type: "spring", stiffness: 380, damping: 34 }}
          className="relative shrink-0 border-t border-human bg-human-bg/90 px-6 py-3 backdrop-blur"
        >
          <span className="pointer-events-none absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-human/60 to-transparent" />
          {task.status === "DRAFT" && <DraftBar task={task} />}
          {task.status === "WAITING_FOR_PLAN_APPROVAL" && <PlanApprovalBar task={task} />}
          {task.status === "WAITING_FOR_HUMAN_APPROVAL" && <WaitingBar task={task} />}
          {task.status === "APPROVED" && <ApprovedBar task={task} />}
          {task.status === "MERGE_CONFLICT" && <ConflictBar task={task} />}
        </motion.div>
      )}
    </AnimatePresence>
  );
}

const rowCls = "flex items-center justify-between gap-4";
const leadCls = "font-semibold text-human";

/* ---- DRAFT ---------------------------------------------------------- */
function DraftBar({ task }: Props) {
  const start = useStartTask();
  const cancel = useCancelTask();

  const run = async (fn: () => Promise<unknown>) => {
    try {
      await fn();
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  return (
    <div className={rowCls}>
      <div className="flex min-w-0 flex-col gap-0.5">
        <span className={leadCls}>草稿尚未启动</span>
        <span className="text-[13px] text-t2">启动后会创建隔离工作树，并交给开发 Agent 运行。</span>
      </div>
      <div className="flex shrink-0 gap-2">
        <Button variant="outline" disabled={cancel.isPending || start.isPending} onClick={() => run(() => cancel.mutateAsync(task.id))}>
          取消任务
        </Button>
        <Button variant="human" disabled={start.isPending || cancel.isPending} onClick={() => run(() => start.mutateAsync(task.id))}>
          {start.isPending ? "启动中…" : "开始任务"}
        </Button>
      </div>
    </div>
  );
}

/* ---- WAITING_FOR_HUMAN_APPROVAL ------------------------------------- */
function WaitingBar({ task }: Props) {
  const rev = task.currentRevision;
  const client = useQueryClient();
  const diff = useDiff(task.id, rev);
  const review = useReview(task.id, rev);
  const approve = useApproveTask();
  const reject = useRejectTask();
  const cancel = useCancelTask();

  const [confirmOpen, setConfirmOpen] = useState(false);
  const [rejectOpen, setRejectOpen] = useState(false);
  const [cancelOpen, setCancelOpen] = useState(false);
  const [reason, setReason] = useState("");

  const payload = diff.data;
  const flaggedCount = payload?.files.filter((f) => f.flagged).length ?? 0;
  const ins = payload?.files.reduce((a, f) => a + f.insertions, 0) ?? 0;
  const del = payload?.files.reduce((a, f) => a + f.deletions, 0) ?? 0;

  const doApprove = async () => {
    if (!payload) return;
    try {
      await approve.mutateAsync({ taskId: task.id, revision: rev, commitSha: payload.commitSha, diffSha256: payload.diffSha256 });
      setConfirmOpen(false);
    } catch (e) {
      const d = toAppError(e);
      if (d.code === "DIFF_STALE") {
        setConfirmOpen(false);
        client.invalidateQueries({ queryKey: qk.diff(task.id, rev) });
        client.invalidateQueries({ queryKey: qk.task(task.id) });
        toast.info("内容已变化，已为你刷新，请重新确认");
      } else {
        toast.error(errorLine(e));
      }
    }
  };

  const doReject = async () => {
    if (!reason.trim()) return;
    try {
      await reject.mutateAsync({ taskId: task.id, revision: rev, reason: reason.trim() });
      setRejectOpen(false);
      setReason("");
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  return (
    <>
      <div className={rowCls}>
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <span className={leadCls}>{review.data?.decision === "pass" ? "审查通过" : "等你批准"}</span>
          {payload && (
            <>
              <span className="text-t2">·</span>
              <CopyText value={payload.commitSha} title="点击复制 commit SHA" className="text-t2">
                {shortSha(payload.commitSha)}
              </CopyText>
              <span className="flex gap-1.5 font-mono">
                <span className="text-ok">+{ins}</span>
                <span className="text-bad">−{del}</span>
              </span>
              {flaggedCount > 0 && <span className="text-[12px] text-human">⚠ {flaggedCount} 个控制面文件</span>}
            </>
          )}
          {review.data?.summary && (
            <span className="max-w-md truncate text-[13px] text-t2">{review.data.summary}</span>
          )}
        </div>
        <div className="flex shrink-0 gap-2">
          <Button variant="outline" onClick={() => setCancelOpen(true)}>取消任务</Button>
          <Button variant="danger" onClick={() => setRejectOpen(true)}>驳回…</Button>
          <Button variant="human" disabled={!payload || approve.isPending} onClick={() => setConfirmOpen(true)}>
            批准并进入合并
          </Button>
        </div>
      </div>

      <Dialog
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        title="确认批准这一轮改动"
        onConfirmKey={doApprove}
        footer={
          <>
            <Button variant="outline" onClick={() => setConfirmOpen(false)}>取消</Button>
            <Button variant="human" onClick={doApprove} disabled={approve.isPending}>
              {approve.isPending ? "批准中…" : "确认批准 (⌘↵)"}
            </Button>
          </>
        }
      >
        {payload && (
          <div className="flex flex-col gap-2 text-[13px]">
            <div className="flex gap-3">
              <span className="w-12 shrink-0 text-t2">commit</span>
              <CopyText value={payload.commitSha}>{shortSha(payload.commitSha)}</CopyText>
            </div>
            <div className="flex gap-3">
              <span className="w-12 shrink-0 text-t2">改动</span>
              <span>
                <span className="font-mono text-ok">+{ins}</span>{" "}
                <span className="font-mono text-bad">−{del}</span>{" "}
                <span className="text-t3">· {payload.files.length} 个文件</span>
              </span>
            </div>
            {flaggedCount > 0 && (
              <div className="flex items-center gap-2 rounded-md border border-human bg-human-bg px-3 py-2 text-human">
                <AlertTriangle className="size-4 shrink-0" />
                本轮修改了 {flaggedCount} 个规则/控制面文件，请确认这是你想要的。
              </div>
            )}
            <p className="text-[12px] text-t2">批准后任务将进入合并流程。</p>
          </div>
        )}
      </Dialog>

      <Dialog
        open={cancelOpen}
        onClose={() => setCancelOpen(false)}
        title="确认取消任务"
        footer={
          <>
            <Button variant="outline" onClick={() => setCancelOpen(false)}>返回</Button>
            <Button
              variant="danger"
              disabled={cancel.isPending}
              onClick={async () => {
                try {
                  await cancel.mutateAsync(task.id);
                  setCancelOpen(false);
                } catch (e) {
                  toast.error(errorLine(e));
                }
              }}
            >
              {cancel.isPending ? "取消中…" : "确认取消"}
            </Button>
          </>
        }
      >
        <p className="text-[13px] leading-relaxed text-t2">
          任务将停止，不会进入合并。已有分支、运行记录和审查结论仍会保留，便于稍后追溯或清理。
        </p>
      </Dialog>

      <Dialog
        open={rejectOpen}
        onClose={() => setRejectOpen(false)}
        title="驳回并要求返工"
        onConfirmKey={doReject}
        footer={
          <>
            <Button variant="outline" onClick={() => setRejectOpen(false)}>取消</Button>
            <Button variant="danger" onClick={doReject} disabled={!reason.trim() || reject.isPending}>
              {reject.isPending ? "提交中…" : "驳回并返工"}
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-2">
          <Label htmlFor="reject-reason">驳回理由（必填，会作为下一轮开发的输入）</Label>
          <Textarea
            id="reject-reason"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            placeholder="说明哪里不对、期望怎么改…"
            autoFocus
          />
        </div>
      </Dialog>
    </>
  );
}

/* ---- APPROVED -------------------------------------------------------- */
function ApprovedBar({ task }: Props) {
  const deliver = useDeliveryStart();
  const refresh = useDeliveryRefresh();
  const markExternal = useMarkMergedExternal();
  const remoteOpen = task.policy.deliveryMode !== "local_merge" && !!task.delivery?.remoteUrl;
  const run = async (fn: () => Promise<unknown>) => {
    try {
      await fn();
    } catch (e) {
      toast.error(errorLine(e));
    }
  };
  return (
    <div className={rowCls}>
      <div className="flex min-w-0 flex-wrap items-center gap-2">
        <span className={leadCls}>已批准 · 待合并</span>
        <span className="text-[13px] text-t2">{remoteOpen ? "PR / MR 已创建，等待远端 CI 与合并状态。" : "选择由 AgentFlow 交付，或你自己合并后标记完成。"}</span>
      </div>
      <div className="flex shrink-0 gap-2">
        {task.policy.deliveryMode === "local_merge" && <Button variant="outline" disabled={markExternal.isPending} onClick={() => run(() => markExternal.mutateAsync(task.id))}>
          我自己合并（标记完成）
        </Button>}
        <Button variant="human" disabled={deliver.isPending || refresh.isPending} onClick={() => run(() => remoteOpen ? refresh.mutateAsync(task.id) : deliver.mutateAsync(task.id))}>
          {deliver.isPending || refresh.isPending ? "处理中…" : remoteOpen ? "刷新 PR / CI" : task.policy.deliveryMode === "local_merge" ? "立即合并" : task.policy.deliveryMode === "github_pr" ? "创建 GitHub PR" : "创建 GitLab MR"}
        </Button>
      </div>
    </div>
  );
}

/* ---- MERGE_CONFLICT -------------------------------------------------- */
function ConflictBar({ task }: Props) {
  const deliver = useDeliveryStart();
  const markExternal = useMarkMergedExternal();
  const run = async (fn: () => Promise<unknown>) => {
    try {
      await fn();
    } catch (e) {
      toast.error(errorLine(e));
    }
  };
  return (
    <div className={rowCls}>
      <div className="flex min-w-0 flex-wrap items-center gap-2">
        <span className={leadCls}>合并冲突</span>
        <span className="text-[13px] text-t2">
          {task.blockedDetail ?? "自动合并遇到冲突。你可以重试，或在本地手动解决后标记完成。"}
        </span>
      </div>
      <div className="flex shrink-0 gap-2">
        <Button variant="outline" disabled={markExternal.isPending} onClick={() => run(() => markExternal.mutateAsync(task.id))}>
          已手动解决，标记完成
        </Button>
        <Button variant="human" disabled={deliver.isPending} onClick={() => run(() => deliver.mutateAsync(task.id))}>
          {deliver.isPending ? "重试中…" : "重试交付"}
        </Button>
      </div>
    </div>
  );
}

/* ---- BLOCKED -------------------------------------------------------- */
function BlockedBar({ task }: Props) {
  const reasonKey = task.blockedReason;
  const copy = reasonKey ? BLOCKED_COPY[reasonKey] : null;
  const resume = useResumeWithGuidance();
  const forceApprove = useForceApprove();
  const cancel = useCancelTask();
  const applyRepair = useApplyRepair();

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
    }
  };

  return (
    <>
      <div className="flex flex-col gap-5">
        <div className="flex min-w-0 flex-col gap-1">
          <span className={leadCls}>{copy?.title ?? "需要你处理"}</span>
          <span className="text-[13px] text-t2">{copy?.explanation ?? task.blockedDetail ?? ""}</span>
          {copy?.detailIsQuestion && task.blockedDetail && (
            <blockquote className="mt-1 rounded-r-md border-l-2 border-human bg-app/60 px-3 py-2 text-[13px] text-t1">
              {task.blockedDetail}
            </blockquote>
          )}
        </div>
        <div className="flex flex-wrap justify-end gap-2">{(copy?.actions ?? ["guidance", "cancel"]).map(renderAction)}</div>
      </div>

      <Dialog
        open={guidanceOpen}
        onClose={() => setGuidanceOpen(false)}
        title={asAnswer ? "回答开发 Agent 的问题" : "补充指引后继续"}
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
          <blockquote className="mb-3 rounded-r-md border-l-2 border-human bg-app/60 px-3 py-2 text-[13px] text-t1">
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
          <p className="text-[13px] text-t2">正在检查工作树和检查点…</p>
        ) : repair.isError ? (
          <p className="text-[13px] text-bad">{errorLine(repair.error)}</p>
        ) : repair.data ? (
          <div className="flex flex-col gap-3 text-[13px]">
            <p className="text-t2">
              工作树：{repair.data.worktreeExists ? "存在" : "缺失"} · 残留改动：{repair.data.residualChanges ? "有" : "无"}
            </p>
            {repair.data.latestCheckpoint && (
              <p className="rounded-md bg-app px-3 py-2 text-t2">
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
