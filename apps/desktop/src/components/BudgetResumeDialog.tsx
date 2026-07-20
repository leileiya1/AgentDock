import { useEffect, useState } from "react";
import type { TaskDetail } from "@/generated/bindings";
import { useBudgetUpdate } from "@/hooks/useGovernance";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { Dialog } from "./Dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface Props {
  task: TaskDetail;
  open: boolean;
  onClose: () => void;
}

/** Budget stops are recoverable, but only with limits above recorded usage. */
export function BudgetResumeDialog({ task, open, onClose }: Props) {
  const update = useBudgetUpdate();
  const [tokens, setTokens] = useState("");
  const [cost, setCost] = useState("");
  const [seconds, setSeconds] = useState("");

  useEffect(() => {
    if (!open) return;
    setTokens(suggest(task.budget.tokenBudget, task.budget.tokensUsed, 10_000, 0));
    setCost(suggest(task.budget.costBudgetUsd, task.budget.costUsd ?? 0, 5, 2));
    setSeconds(suggest(task.budget.timeBudgetSecs, task.budget.timeUsedSecs, 1_800, 0));
  }, [open, task.budget]);

  const submit = async () => {
    try {
      await update.mutateAsync({
        taskId: task.id,
        limits: {
          tokenBudget: optionalNumber(tokens),
          costBudgetUsd: optionalNumber(cost),
          timeBudgetSecs: optionalNumber(seconds),
        },
      });
      onClose();
      toast.info("预算已更新，任务从中断阶段继续");
    } catch (error) {
      toast.error(errorLine(error));
    }
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="调整预算并继续"
      onConfirmKey={submit}
      footer={
        <>
          <Button variant="outline" onClick={onClose}>取消</Button>
          <Button variant="human" disabled={update.isPending} onClick={submit}>
            {update.isPending ? "恢复中…" : "保存并继续"}
          </Button>
        </>
      }
    >
      <p className="mb-4 text-[12px] leading-relaxed text-t3">
        新上限必须高于已用量；留空表示该项不设上限。任务会从预算中断前的安全调度阶段恢复。
      </p>
      <div className="grid grid-cols-3 gap-3">
        <BudgetField label={`Token · 已用 ${task.budget.tokensUsed}`} value={tokens} onChange={setTokens} step="1000" />
        <BudgetField label={`费用 · 已用 $${(task.budget.costUsd ?? 0).toFixed(4)}`} value={cost} onChange={setCost} step="0.01" />
        <BudgetField label={`时间 · 已用 ${task.budget.timeUsedSecs} 秒`} value={seconds} onChange={setSeconds} step="60" />
      </div>
    </Dialog>
  );
}

function BudgetField({ label, value, onChange, step }: { label: string; value: string; onChange: (value: string) => void; step: string }) {
  return <div className="flex flex-col gap-2"><Label>{label}</Label><Input type="number" min="0" step={step} value={value} onChange={(event) => onChange(event.target.value)} placeholder="不限" /></div>;
}

function optionalNumber(value: string): number | null {
  return value.trim() === "" ? null : Number(value);
}

function suggest(limit: number | null | undefined, used: number, floor: number, digits: number): string {
  const value = Math.max(limit ?? 0, used * 1.25, floor);
  return digits === 0 ? String(Math.ceil(value)) : value.toFixed(digits);
}
