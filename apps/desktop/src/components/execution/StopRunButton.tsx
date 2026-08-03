import { useState } from "react";
import { Square } from "lucide-react";
import { useCancelTask } from "@/hooks/useTasks";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { Dialog } from "@/components/Dialog";
import { Button } from "@/components/ui/button";

/**
 * 停止后确实会保留的东西 (§17)。措辞必须与后台 `cancel` 的真实语义一致：
 * 已提交的轮次进入分支历史，运行日志与审查记录留档；而**当前这一轮尚未提交的改动
 * 会随隔离工作区一起被清理**，任务标记为已取消（终态，不可从当前状态续跑）。
 * 只承诺后台真正做到的事——不谎称能保存在改文件或恢复会话。
 */
export const STOP_PRESERVED_ITEMS = [
  "已完成轮次的提交与分支历史",
  "运行日志与 Agent 输出",
  "审查结论与任务记录",
] as const;

/**
 * 纯展示的停止确认弹窗。容器负责接线，弹窗只描述「会保留 / 会丢弃什么」并给两个选择，
 * 便于像其它 governance 弹窗一样用 renderToStaticMarkup 快照测试。
 */
export function StopConfirmDialog({
  open,
  pending,
  onClose,
  onConfirm,
}: {
  open: boolean;
  pending: boolean;
  onClose: () => void;
  onConfirm: () => void;
}) {
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="停止当前正在运行的 Agent？"
      footer={
        <>
          <Button variant="outline" onClick={onClose}>
            继续运行
          </Button>
          <Button variant="danger" disabled={pending} onClick={onConfirm}>
            {pending ? "停止中…" : "停止任务"}
          </Button>
        </>
      }
    >
      <div className="space-y-2 text-body leading-relaxed text-t2">
        <p>停止后会保留：</p>
        <ul className="list-disc space-y-0.5 pl-5">
          {STOP_PRESERVED_ITEMS.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
        <p>
          当前这一轮尚未提交的改动会被丢弃，任务将标记为「已取消」；隔离工作区会被清理，
          你的原始项目不受影响。
        </p>
      </div>
    </Dialog>
  );
}

/**
 * 运行中任务的「停止」入口 (清单 §14 停止按钮 / §17 用户中途停止)。
 * 后台会中止进程组、留存已完成轮次与运行记录，并清理隔离工作区；任务进入终态「已取消」。
 * 这是终止而非暂停——不谎称能从当前状态续跑（清单 §40：没有的能力就不要展示）。
 */
export function StopRunButton({ taskId }: { taskId: string }) {
  const cancel = useCancelTask();
  const [open, setOpen] = useState(false);

  return (
    <>
      <Button
        variant="outline"
        size="sm"
        className="shrink-0"
        disabled={cancel.isPending}
        onClick={() => setOpen(true)}
      >
        <Square className="size-3.5" aria-hidden /> 停止
      </Button>

      <StopConfirmDialog
        open={open}
        pending={cancel.isPending}
        onClose={() => setOpen(false)}
        onConfirm={async () => {
          try {
            await cancel.mutateAsync(taskId);
            setOpen(false);
          } catch (e) {
            toast.error(errorLine(e));
          }
        }}
      />
    </>
  );
}
