import { useEffect, useRef, useState } from "react";
import { ShieldAlert } from "lucide-react";
import type { TaskStatus } from "@/generated/bindings";
import { usePermissionRequests } from "@/hooks/usePermissions";
import { pendingCount } from "@/lib/permission/model";
import { Button } from "@/components/ui/button";
import { PermissionRequestDialog } from "@/components/permission/PermissionRequestDialog";

/**
 * 任务级权限入口 (06 §9 第 4/16/17 条)。挂在任务详情底部：有未处理请求时常驻「等待你授权」
 * 条并在首次出现时自动弹窗；关闭后仍可从条上重新打开（后端持久化，剩余有效期照常倒计时）。
 */
export function PermissionGate({
  taskId,
  projectId,
  taskStatus,
}: {
  taskId: string;
  projectId: string | undefined;
  taskStatus: TaskStatus;
}) {
  const requests = usePermissionRequests(taskId);
  const pending = pendingCount(requests.data);
  const [open, setOpen] = useState(false);

  // 待处理集合从无到有时自动弹出一次；用户关闭后不再打扰，直到出现新的一批。
  const pendingKey = (requests.data ?? [])
    .filter((r) => r.status === "pending")
    .map((r) => r.id)
    .join(",");
  const lastKey = useRef("");
  useEffect(() => {
    if (pendingKey && pendingKey !== lastKey.current) setOpen(true);
    lastKey.current = pendingKey;
  }, [pendingKey]);

  if (pending === 0) return null;

  return (
    <>
      <div
        role="status"
        className="flex shrink-0 items-center justify-between gap-3 border-t border-status-human/50 bg-status-human-bg px-4 py-2.5 text-body text-status-human"
      >
        <span className="flex min-w-0 items-center gap-2">
          <ShieldAlert className="size-4 shrink-0" aria-hidden />
          <span className="min-w-0">
            {taskStatus === "BLOCKED"
              ? `AgentFlow 已停止 Provider，${pending} 项执行权限等待你授权`
              : `${pending} 项执行权限等待你授权`}
          </span>
        </span>
        <Button variant="human" size="sm" onClick={() => setOpen(true)}>
          查看并处理
        </Button>
      </div>
      <PermissionRequestDialog
        taskId={taskId}
        projectId={projectId}
        requests={requests.data ?? []}
        open={open}
        onClose={() => setOpen(false)}
      />
    </>
  );
}
