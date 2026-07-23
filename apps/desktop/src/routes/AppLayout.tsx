import { useEffect } from "react";
import { Outlet, useLocation, useMatch } from "react-router-dom";
import { Sidebar } from "@/components/Sidebar";
import { Toaster } from "@/components/Toaster";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { NewTaskDialog } from "@/routes/NewTaskDialog";
import { CloseGuard } from "@/components/CloseGuard";
import { FullAccessBanner } from "@/components/FullAccessBanner";
import { SchedulerBanner } from "@/components/SchedulerBanner";
import { useGlobalEvents } from "@/hooks/useGlobalEvents";
import { useUiStore } from "@/stores/uiStore";

export function AppLayout() {
  const location = useLocation();
  const listMatch = useMatch("/p/:projectId");
  const detailMatch = useMatch("/p/:projectId/t/:taskId");
  const openNewTask = useUiStore((s) => s.openNewTask);

  useGlobalEvents();

  // Cmd/Ctrl+N → new task for the current project (02 §9).
  useEffect(() => {
    const projectId = listMatch?.params.projectId ?? detailMatch?.params.projectId;
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && (e.key === "n" || e.key === "N")) {
        if (projectId) {
          e.preventDefault();
          openNewTask(projectId);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [listMatch, detailMatch, openNewTask]);

  return (
    <div className="flex h-dvh min-h-0 w-full overflow-hidden">
      <Sidebar />
      <main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
        <FullAccessBanner />
        <SchedulerBanner />
        <ErrorBoundary resetKey={location.pathname}>
          <Outlet />
        </ErrorBoundary>
      </main>
      <NewTaskDialog />
      <CloseGuard />
      <Toaster />
    </div>
  );
}
