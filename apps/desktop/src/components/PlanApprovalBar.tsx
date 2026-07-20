import { useState } from "react";
import type { TaskDetail } from "@/generated/bindings";
import { useCancelTask } from "@/hooks/useTasks";
import { usePlanApprove, usePlanReject } from "@/hooks/useGovernance";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { Dialog } from "@/components/Dialog";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/input";

export function PlanApprovalBar({ task }: { task: TaskDetail }) {
  const approve = usePlanApprove();
  const reject = usePlanReject();
  const cancel = useCancelTask();
  const [planOpen, setPlanOpen] = useState(false);
  const [rejectOpen, setRejectOpen] = useState(false);
  const [cancelOpen, setCancelOpen] = useState(false);
  const [reason, setReason] = useState("");
  const plan = task.plan;

  const approvePlan = async () => {
    if (!plan) return;
    try {
      await approve.mutateAsync({ taskId: task.id, planId: plan.id });
      setPlanOpen(false);
    } catch (error) {
      toast.error(errorLine(error));
    }
  };

  const rejectPlan = async () => {
    if (!plan || !reason.trim()) return;
    try {
      await reject.mutateAsync({ taskId: task.id, planId: plan.id, reason: reason.trim() });
      setRejectOpen(false);
      setReason("");
    } catch (error) {
      toast.error(errorLine(error));
    }
  };

  return (
    <>
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0">
          <div className="font-semibold text-human">编码前计划等你审批</div>
          <div className="max-w-2xl truncate text-[13px] text-t2">
            {plan?.summary ?? "计划正在同步…"}{plan ? ` · ${plan.steps.length} 个步骤` : ""}
          </div>
        </div>
        <div className="flex shrink-0 gap-2">
          <Button variant="outline" onClick={() => setCancelOpen(true)}>取消任务</Button>
          <Button variant="danger" disabled={!plan} onClick={() => setRejectOpen(true)}>驳回计划…</Button>
          <Button variant="human" disabled={!plan} onClick={() => setPlanOpen(true)}>查看并批准</Button>
        </div>
      </div>

      <Dialog
        open={planOpen}
        onClose={() => setPlanOpen(false)}
        title={`编码计划 v${plan?.version ?? 1}`}
        width={680}
        onConfirmKey={approvePlan}
        footer={
          <>
            <Button variant="outline" onClick={() => setPlanOpen(false)}>返回</Button>
            <Button variant="human" disabled={approve.isPending || !plan} onClick={approvePlan}>
              {approve.isPending ? "批准中…" : "批准并开始编码"}
            </Button>
          </>
        }
      >
        {plan && (
          <div className="flex flex-col gap-4 text-[13px]">
            <p className="leading-relaxed text-t1">{plan.summary}</p>
            <ol className="flex flex-col gap-2">
              {plan.steps.map((step, index) => (
                <li key={`${step.title}-${index}`} className="rounded-md border border-line bg-app/60 p-3">
                  <div className="font-medium text-t1">{index + 1}. {step.title}</div>
                  <div className="mt-1 leading-relaxed text-t2">{step.detail}</div>
                  {step.validation && <div className="mt-1 text-[12px] text-ok">验证：{step.validation}</div>}
                </li>
              ))}
            </ol>
            <div className="rounded-md border border-line bg-raised p-3">
              <div className="font-medium text-t1">允许修改的路径</div>
              <div className="mt-2 flex flex-wrap gap-1.5">
                {plan.allowedPaths.map((path) => (
                  <span key={path} className="rounded bg-app px-2 py-1 font-mono text-[11px] text-t2">{path}</span>
                ))}
              </div>
              <p className="mt-2 text-[11px] text-t3">Agent 修改范围超出这些路径时，本轮会被重置并重新请求你的计划审批。</p>
            </div>
            {plan.risks.length > 0 && (
              <div className="rounded-md border border-human/40 bg-human-bg p-3">
                <div className="font-medium text-human">计划风险</div>
                <ul className="mt-1 list-disc space-y-1 pl-5 text-t2">
                  {plan.risks.map((risk) => <li key={risk}>{risk}</li>)}
                </ul>
              </div>
            )}
          </div>
        )}
      </Dialog>

      <Dialog
        open={rejectOpen}
        onClose={() => setRejectOpen(false)}
        title="驳回并重新拟定计划"
        onConfirmKey={rejectPlan}
        footer={
          <>
            <Button variant="outline" onClick={() => setRejectOpen(false)}>返回</Button>
            <Button variant="danger" disabled={!reason.trim() || reject.isPending} onClick={rejectPlan}>重新拟定</Button>
          </>
        }
      >
        <div className="flex flex-col gap-2">
          <Label htmlFor="plan-reject-reason">需要调整的地方</Label>
          <Textarea id="plan-reject-reason" value={reason} onChange={(event) => setReason(event.target.value)} autoFocus />
        </div>
      </Dialog>

      <Dialog
        open={cancelOpen}
        onClose={() => setCancelOpen(false)}
        title="确认取消任务"
        footer={
          <>
            <Button variant="outline" onClick={() => setCancelOpen(false)}>返回</Button>
            <Button variant="danger" disabled={cancel.isPending} onClick={async () => {
              try { await cancel.mutateAsync(task.id); setCancelOpen(false); }
              catch (error) { toast.error(errorLine(error)); }
            }}>确认取消</Button>
          </>
        }
      >
        <p className="text-[13px] text-t2">任务不会进入编码，计划和审计记录仍会保留。</p>
      </Dialog>
    </>
  );
}
