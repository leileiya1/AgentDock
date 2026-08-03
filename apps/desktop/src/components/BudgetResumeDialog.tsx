import { useEffect, useState } from "react";
import type { TaskDetail } from "@/generated/bindings";
import { useBudgetUpdate } from "@/hooks/useGovernance";
import { budgetView, suggestedLimit } from "@/lib/governance/budget";
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

  const view = budgetView(task.budget);
  const metric = (key: "tokens" | "cost" | "time") => view.metrics.find((m) => m.key === key)!;

  useEffect(() => {
    if (!open) return;
    // 用量未知时不给建议值——用 0 推算出的上限会误导用户 (05 §6.2)。
    const propose = (key: "tokens" | "cost" | "time", digits: number) => {
      const suggestion = suggestedLimit(metric(key), 1.25);
      if (suggestion == null) return "";
      return digits === 0 ? String(Math.ceil(suggestion)) : suggestion.toFixed(digits);
    };
    setTokens(propose("tokens", 0));
    setCost(propose("cost", 2));
    setSeconds(propose("time", 0));
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
      <p className="mb-3 text-meta leading-relaxed text-t3">
        新上限必须高于已用量；留空表示该项不设上限。
        任务会从预算中断前保存的检查点继续，已完成的开发和验证不会重跑。
      </p>
      {view.hasUnknown && (
        <p className="mb-3 rounded-control border border-caution/50 bg-caution-bg px-3 py-2 text-meta leading-relaxed text-t1">
          有维度的用量 Provider 没有提供，无法据此推算新上限，请自行填写。
        </p>
      )}
      <div className="grid grid-cols-3 gap-3">
        <BudgetField label={`Token · 已用 ${metric("tokens").display}`} value={tokens} onChange={setTokens} step="1000" />
        <BudgetField label={`费用 · 已用 ${metric("cost").display}`} value={cost} onChange={setCost} step="0.01" />
        <BudgetField label={`时间 · 已用 ${metric("time").display}`} value={seconds} onChange={setSeconds} step="60" />
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
