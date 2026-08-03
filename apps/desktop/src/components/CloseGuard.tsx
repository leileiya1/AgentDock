import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useQueryClient } from "@tanstack/react-query";
import type { TaskSummary } from "@/generated/bindings";
import { countActiveTasks } from "@/lib/execution/closeGuard";
import { Dialog } from "@/components/Dialog";
import { Button } from "@/components/ui/button";

/**
 * §40 关闭窗口告知弹窗（纯展示）。任务由分离的后台服务托管，关窗口不会打断它们，因此如实告知
 * 「会在后台继续」，并只给后台服务真正支持的两个选择——不提供做不到的「暂停并保存」(对照 §40)。
 */
export function CloseGuardDialog({
  open,
  activeCount,
  onKeepRunning,
  onCancel,
}: {
  open: boolean;
  activeCount: number;
  onKeepRunning: () => void;
  onCancel: () => void;
}) {
  return (
    <Dialog
      open={open}
      onClose={onCancel}
      title="关闭 AgentFlow？"
      footer={
        <>
          <Button variant="outline" onClick={onCancel}>
            取消关闭
          </Button>
          <Button variant="human" onClick={onKeepRunning}>
            后台继续并关闭
          </Button>
        </>
      }
    >
      <p className="text-body leading-relaxed text-t2">
        当前有 {activeCount} 个任务正在运行。后台服务会保持运行，关闭窗口后它们会继续执行，稍后重新打开
        AgentFlow 即可查看进度，不会丢失已完成的工作。
      </p>
    </Dialog>
  );
}

/**
 * Window-close guard (§40). In the packaged Tauri app, intercept the window-close request; if any
 * task is actively running, inform the user that work continues in the background before closing.
 * Inert in the browser dev shell (no __TAURI_INTERNALS__), and closes silently when nothing runs.
 */
export function CloseGuard() {
  const queryClient = useQueryClient();
  const [activeCount, setActiveCount] = useState<number | null>(null);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void (async () => {
      const appWindow = getCurrentWindow();
      const handle = await appWindow.onCloseRequested((event) => {
        const lists = queryClient
          .getQueriesData<TaskSummary[]>({ queryKey: ["tasks"] })
          .map(([, data]) => data);
        const running = countActiveTasks(lists);
        if (running === 0) return; // nothing in flight → let the window close normally
        event.preventDefault(); // hold the close until the user confirms
        setActiveCount(running);
      });
      if (disposed) handle();
      else unlisten = handle;
    })();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [queryClient]);

  return (
    <CloseGuardDialog
      open={activeCount !== null}
      activeCount={activeCount ?? 0}
      onCancel={() => setActiveCount(null)}
      // Confirmed close bypasses the guard: destroy() closes without re-emitting close-requested.
      onKeepRunning={() => void getCurrentWindow().destroy()}
    />
  );
}
