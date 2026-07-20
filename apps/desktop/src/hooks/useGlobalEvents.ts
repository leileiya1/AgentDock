import { useQueryClient } from "@tanstack/react-query";
import type { RunSummary, TaskSummary } from "@/generated/bindings";
import { qk } from "@/lib/queryKeys";
import { useTauriEvent } from "@/lib/tauriEvents";
import { toast } from "@/stores/toastStore";
import { errorLine } from "@/copy/errors";

/**
 * App-wide event → cache wiring (02 §5). Mounted once at the root.
 * task:changed does a precise row setQueryData + detail invalidate; run:* and
 * env:changed keep their caches fresh; app:error surfaces a toast.
 */
export function useGlobalEvents(): void {
  const client = useQueryClient();

  useTauriEvent("task:changed", ({ task }) => {
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
    client.invalidateQueries({ queryKey: qk.events(task.id) });
  });

  useTauriEvent("task:removed", ({ taskId, projectId }) => {
    client.setQueryData<TaskSummary[]>(qk.tasks(projectId), (prev) =>
      prev?.filter((task) => task.id !== taskId)
    );
    client.removeQueries({ queryKey: qk.task(taskId) });
    client.removeQueries({ queryKey: qk.runs(taskId) });
    client.removeQueries({ queryKey: qk.events(taskId) });
  });

  const onRun = (run: RunSummary) => {
    client.invalidateQueries({ queryKey: qk.runs(run.taskId) });
    client.invalidateQueries({ queryKey: qk.events(run.taskId) });
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
