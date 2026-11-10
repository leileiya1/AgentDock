import { useEffect } from "react";
import { Outlet, useLocation, useMatch, useNavigate } from "react-router-dom";
import { Sidebar } from "@/components/Sidebar";
import { Toaster } from "@/components/Toaster";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { NewTaskDialog } from "@/routes/NewTaskDialog";
import { CloseGuard } from "@/components/CloseGuard";
import { FullAccessBanner } from "@/components/FullAccessBanner";
import { SchedulerBanner } from "@/components/SchedulerBanner";
import { useGlobalEvents } from "@/hooks/useGlobalEvents";
import { useUiStore } from "@/stores/uiStore";
import { CommandPalette } from "@/components/palette/CommandPalette";
import { AwayRecap } from "@/components/recap/AwayRecap";
import { Fireworks } from "@/components/celebrate/Fireworks";
import { ShortcutSheet } from "@/components/ShortcutSheet";
import { WeeklyReportDialog } from "@/components/weekly/WeeklyReportDialog";

export function AppLayout() {
  const location = useLocation();
  const listMatch = useMatch("/p/:projectId");
  const detailMatch = useMatch("/p/:projectId/t/:taskId");
  const openNewTask = useUiStore((s) => s.openNewTask);
  const navigate = useNavigate();
  const paletteOpen = useUiStore((s) => s.paletteOpen);
  const setPaletteOpen = useUiStore((s) => s.setPaletteOpen);
  const currentProjectId = listMatch?.params.projectId ?? detailMatch?.params.projectId;

  useGlobalEvents();

  // Native desktop shortcuts stay global, but never steal keystrokes from text fields.
  useEffect(() => {
    const projectId = listMatch?.params.projectId ?? detailMatch?.params.projectId;
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key === ",") {
        e.preventDefault();
        navigate("/settings");
        return;
      }
      if (mod && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        setPaletteOpen((o) => !o);
        return;
      }
      if (mod && e.key === "[") {
        e.preventDefault();
        navigate(-1);
        return;
      }
      if ((e.metaKey || e.ctrlKey) && (e.key === "n" || e.key === "N")) {
        if (projectId) {
          e.preventDefault();
          openNewTask(projectId);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [listMatch, detailMatch, openNewTask, navigate, setPaletteOpen]);

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
      <AwayRecap />
      <Fireworks />
      <ShortcutSheet />
      <WeeklyReportDialog />
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} currentProjectId={currentProjectId} />
    </div>
  );
}
