import { useQueryClient } from "@tanstack/react-query";
import type { RunSummary, TaskSummary } from "@/generated/bindings";
import { qk } from "@/lib/queryKeys";
import { useTauriEvent } from "@/lib/tauriEvents";
import { toast } from "@/stores/toastStore";
import { errorLine } from "@/copy/errors";
import { groupForStatus } from "@/copy/status";
import { chimeEnabled, playChime } from "@/lib/chime";
import { useCelebrateStore } from "@/stores/celebrateStore";

/**
 * App-wide event → cache wiring (02 §5). Mounted once at the root.
 * task:changed does a precise row setQueryData + detail invalidate; run:* and
 * env:changed keep their caches fresh; app:error surfaces a toast.
 */
export function useGlobalEvents(): void {
  const client = useQueryClient();

  useTauriEvent("task:changed", ({ task }) => {
    // 8-bit 提示音：只在状态组真的变了的时候响（开关在 ⌘K 里）。
    const before = client.getQueryData<TaskSummary[]>(qk.tasks(task.projectId))?.find((t) => t.id === task.id);
    if (chimeEnabled()) {
      const kind = chimeFor(before?.status, task.status);
      if (kind) playChime(kind);
    }
    // 合并成功放烟花，开发它的机器人在小屋里蹦一蹦。
    if (task.status === "MERGED" && before?.status !== "MERGED") {
      useCelebrateStore.getState().celebrate({
        text: `TASK-${String(task.seq).padStart(3, "0")}「${task.title}」合并上线！`,
        projectId: task.projectId,
        agent: task.developerAgent,
      });
    }
    // Precise row update in the list.
    client.setQueryData<TaskSummary[]>(qk.tasks(task.projectId), (prev) => {
      if (!prev) return prev;
      const idx = prev.findIndex((t) => t.id === task.id);
      if (idx === -1) return [task, ...prev];
      const next = prev.slice();
      next[idx] = task;
      return next;
    });
    // Detail refetches for the richer TaskDetail fields.
    client.invalidateQueries({ queryKey: qk.task(task.id) });
    client.invalidateQueries({ queryKey: qk.queue(task.id) });
    client.invalidateQueries({ queryKey: qk.events(task.id) });
    // A task entering/leaving BLOCKED(permission_required) surfaces or clears
    // pending 权限请求 (06 §9 第 4 条). Event-driven refresh keeps the 授权弹窗
    // and 任务树「等待你授权」标记live without polling.
    client.invalidateQueries({ queryKey: qk.permissionRequests(task.id) });
  });

  useTauriEvent("task:removed", ({ taskId, projectId }) => {
    client.setQueryData<TaskSummary[]>(qk.tasks(projectId), (prev) =>
      prev?.filter((task) => task.id !== taskId)
    );
    client.removeQueries({ queryKey: qk.task(taskId) });
    client.removeQueries({ queryKey: qk.queue(taskId) });
    client.removeQueries({ queryKey: qk.runs(taskId) });
    client.removeQueries({ queryKey: qk.events(taskId) });
  });

  const onRun = (run: RunSummary) => {
    client.invalidateQueries({ queryKey: qk.runs(run.taskId) });
    client.invalidateQueries({ queryKey: qk.events(run.taskId) });
    client.invalidateQueries({ queryKey: qk.queue(run.taskId) });
  };
  useTauriEvent("run:started", onRun);
  useTauriEvent("run:finished", onRun);

  useTauriEvent("env:changed", (env) => {
    client.setQueryData(qk.env, env);
    client.invalidateQueries({ queryKey: qk.onboarding });
  });

  useTauriEvent("app:error", (payload) => {
    toast.error(errorLine({ code: payload.code, message: payload.message, detail: null }));
  });
}

/** 状态变化 → 该响哪段提示音；没变或不值得打扰就不响。 */
export function chimeFor(before: TaskSummary["status"] | undefined, after: TaskSummary["status"]): "done" | "attention" | "failed" | null {
  if (before === after) return null;
  if (after === "MERGED") return "done";
  if (after === "MERGE_CONFLICT" || after === "ROLLED_BACK") return "failed";
  if (groupForStatus(after) === "attention" && (!before || groupForStatus(before) !== "attention")) return "attention";
  return null;
}
