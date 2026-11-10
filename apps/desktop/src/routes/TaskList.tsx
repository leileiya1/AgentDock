import { useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ChevronRight, ListFilter } from "lucide-react";
import type { TaskSummary } from "@/generated/bindings";
import { useProjectSummaries, useProjects } from "@/hooks/useProjects";
import { useTasks } from "@/hooks/useTasks";
import { useListKeys } from "@/hooks/useListKeys";
import { useUiStore } from "@/stores/uiStore";
import { relativeTime, taskCode } from "@/lib/format";
import { buildHomeTaskSections, type HomeView } from "@/lib/homeTasks";
import { cn } from "@/lib/utils";
import { StateBadge } from "@/components/StateBadge";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { Badge } from "@/components/ui/badge";
import { AuditExportDialog } from "@/components/AuditExportDialog";
import { PermissionProjectSummary } from "@/components/permission/PermissionProjectSummary";
import { ProjectOverview } from "@/components/home/ProjectOverview";
import { ProjectEnvironmentStatus } from "@/components/home/ProjectEnvironmentStatus";
import { HomeEmpty } from "@/components/home/HomeEmpty";
import { AttentionCenter } from "@/components/home/AttentionCenter";
import { SquadStrip } from "@/components/home/SquadStrip";
import { StudioRoom } from "@/components/studio/StudioRoom";
import { DailyPaper } from "@/components/home/DailyPaper";
import { PixelBot } from "@/components/pixel/PixelBot";
import { PixelIcon } from "@/components/pixel/PixelIcon";
import { relayStage, taskAvatar } from "@/lib/squad";
import { isTaskExecuting } from "@/lib/taskStatus";
import { agentLabel } from "@/copy/agents";
import { STATUS_COPY } from "@/copy/status";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";

export function TaskList() {
  const { projectId } = useParams();
  const navigate = useNavigate();
  const projects = useProjects();
  const tasks = useTasks(projectId);
  const summaries = useProjectSummaries();
  const openNewTask = useUiStore((s) => s.openNewTask);
  const [homeView, setHomeView] = useState<HomeView>("attention");
  const [showDone, setShowDone] = useState(false);
  const [auditOpen, setAuditOpen] = useState(false);
  const [query, setQuery] = useState("");
  const density = useUiStore((s) => s.densityMode);
  const toggleDensity = useUiStore((s) => s.toggleDensity);
  const homeMode = useUiStore((s) => s.homeMode);
  const setHomeMode = useUiStore((s) => s.setHomeMode);
  const studio = homeMode === "studio" && (tasks.data?.length ?? 0) > 0;

  const project = projects.data?.find((item) => item.id === projectId);
  const filteredTasks = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) return tasks.data ?? [];
    return (tasks.data ?? []).filter((task) => [
      taskCode(task.seq), task.title, agentLabel(task.developerAgent), agentLabel(task.reviewerAgent), STATUS_COPY[task.status].label,
    ].some((value) => value.toLocaleLowerCase().includes(normalized)));
  }, [tasks.data, query]);
  const sections = useMemo(() => buildHomeTaskSections(filteredTasks), [filteredTasks]);

  useListKeys();
  const onRow = (task: TaskSummary) => navigate(`/p/${projectId}/t/${task.id}`);
  const newTask = () => projectId && openNewTask(projectId);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto px-6 py-6">
        <div className="mx-auto flex max-w-5xl flex-col gap-5">
          {projectId && (
            <ProjectOverview
              project={project}
              counts={sections.counts}
              view={homeView}
              onViewChange={setHomeView}
              onNew={newTask}
              onExport={() => setAuditOpen(true)}
              environmentSlot={<ProjectEnvironmentStatus onOpen={() => navigate("/settings")} />}
              permissionSlot={<PermissionProjectSummary projectId={projectId} tasks={tasks.data} />}
              showStats={sections.counts.all > 0 && !studio}
              squadSlot={(tasks.data?.length ?? 0) > 0 && !studio ? <SquadStrip tasks={tasks.data ?? []} onShowAttention={() => setHomeView("attention")} /> : undefined}
            />
          )}

          {(tasks.data?.length ?? 0) > 0 && (
            <DailyPaper
              tasks={tasks.data ?? []}
              failing={summaries.data?.find((s) => s.projectId === projectId)?.failing ?? 0}
              onOpenTask={(taskId) => navigate(`/p/${projectId}/t/${taskId}`)}
            />
          )}

          {(tasks.data?.length ?? 0) > 0 && (
            <div className="flex items-center gap-2">
              <div className="flex shrink-0 rounded-section bg-status-idle-bg p-1" role="radiogroup" aria-label="首页视图">
                {(["list", "studio"] as const).map((mode) => (
                  <button
                    key={mode}
                    type="button"
                    role="radio"
                    aria-checked={homeMode === mode}
                    onClick={() => setHomeMode(mode)}
                    className={cn(
                      "flex h-8 items-center gap-1.5 rounded-control px-3 text-meta font-bold transition-colors",
                      homeMode === mode ? "bg-raised text-t1 shadow-press" : "text-t2 hover:text-t1"
                    )}
                  >
                    <PixelIcon name={mode === "list" ? "list" : "squad"} size={11} />
                    {mode === "list" ? "清单" : "小屋"}
                  </button>
                ))}
              </div>
              {!studio && (
              <div className="relative min-w-0 flex-1" role="search">
                <PixelIcon name="search" size={12} className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-t3" />
                <Input
                  id="task-search"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  className="pl-9"
                  aria-label="搜索任务"
                  placeholder="搜索 TASK、标题、Provider 或状态"
                />
              </div>
              )}
              {!studio && (
                <Button variant="outline" onClick={toggleDensity} title="切换任务列表、执行轨道和日志的行距">
                  <ListFilter className="size-4" /> {density === "compact" ? "舒适" : "紧凑"}
                </Button>
              )}
            </div>
          )}

          {tasks.isLoading ? (
            <SkeletonRows rows={5} />
          ) : tasks.isError ? (
            <ErrorState error={tasks.error} onRetry={() => tasks.refetch()} />
          ) : sections.counts.all === 0 ? (
            <HomeEmpty onNew={newTask} />
          ) : studio ? (
            <>
              <StudioRoom tasks={tasks.data ?? []} onOpenTask={(taskId) => navigate(`/p/${projectId}/t/${taskId}`)} />
              {sections.attention.length > 0 && <AttentionCenter tasks={sections.attention} onOpen={onRow} density={density} />}
            </>
          ) : (
            <HomeViewContent
              view={homeView}
              sections={sections}
              showDone={showDone}
              onToggleDone={() => setShowDone((current) => !current)}
              onRow={onRow}
              density={density}
            />
          )}
        </div>
      </div>

      {projectId && (
        <AuditExportDialog
          open={auditOpen}
          onClose={() => setAuditOpen(false)}
          projectId={projectId}
          tasks={tasks.data ?? []}
        />
      )}
    </div>
  );
}

function HomeViewContent({
  view,
  sections,
  showDone,
  onToggleDone,
  onRow,
  density,
}: {
  view: HomeView;
  sections: ReturnType<typeof buildHomeTaskSections>;
  showDone: boolean;
  onToggleDone: () => void;
  onRow: (task: TaskSummary) => void;
  density: "comfortable" | "compact";
}) {
  if (view === "attention") {
    return <AttentionCenter tasks={sections.attention} onOpen={onRow} density={density} />;
  }

  if (view === "running") {
    return (
      <TaskGroup
        title="运行中"
        tasks={sections.running}
        onRow={onRow}
        emptyText="当前没有正在执行的任务。草稿和待开始任务不会计入运行中。"
        density={density}
      />
    );
  }

  if (view === "done") {
    return <TaskGroup title="已完结" tasks={sections.done} onRow={onRow} emptyText="还没有已完结的任务。" density={density} />;
  }

  return (
    <div className="flex flex-col gap-6">
      <AttentionCenter tasks={sections.attention} onOpen={onRow} density={density} />
      <TaskGroup title="进行中与待开始" tasks={sections.open} onRow={onRow} emptyText="当前没有进行中或待开始的任务。" density={density} />
      {sections.done.length > 0 && (
        <section>
          <button
            type="button"
            onClick={onToggleDone}
            className="mb-2 flex items-center gap-1.5 px-1 text-meta font-semibold uppercase tracking-wider text-t3 transition-colors hover:text-t2"
          >
            <ChevronRight className={cn("size-3.5 transition-transform duration-200", showDone && "rotate-90")} />
            最近完成
            <Badge>{sections.done.length}</Badge>
          </button>
          {showDone && (
            <div className="flex flex-col overflow-hidden rounded-card border border-line bg-raised">
              {sections.done.slice(0, 5).map((task) => (
                <TaskRow key={task.id} task={task} onClick={() => onRow(task)} density={density} />
              ))}
            </div>
          )}
        </section>
      )}
    </div>
  );
}

function TaskGroup({
  title,
  tasks,
  onRow,
  emptyText,
  density,
}: {
  title: string;
  tasks: TaskSummary[];
  onRow: (task: TaskSummary) => void;
  emptyText: string;
  density: "comfortable" | "compact";
}) {
  return (
    <section>
      <div className="mb-2 flex items-center gap-2 px-1 text-section font-bold text-t1">
        {title}
        <Badge>{tasks.length}</Badge>
      </div>
      {tasks.length === 0 ? (
        <div className="flex items-center justify-center gap-3 rounded-card border border-dashed border-line-strong bg-raised/60 px-4 py-5 text-body text-t2">
          <PixelBot member="runner" mode="idle" size={28} decorative />
          {emptyText}
        </div>
      ) : (
        <div className="flex flex-col overflow-hidden rounded-card border border-line bg-raised">
          {tasks.map((task) => (
            <TaskRow key={task.id} task={task} onClick={() => onRow(task)} density={density} />
          ))}
        </div>
      )}
    </section>
  );
}

function TaskRow({ task, onClick, density }: { task: TaskSummary; onClick: () => void; density: "comfortable" | "compact" }) {
  const avatar = taskAvatar(task);
  const stage = relayStage(task);
  const executing = isTaskExecuting(task.status);
  return (
    <button
      type="button"
      data-task-row
      onClick={onClick}
      className={cn(
        "group flex w-full animate-row-in items-center gap-3.5 border-b border-line/60 px-4 text-left transition-colors last:border-b-0 hover:bg-app focus-visible:bg-app focus-visible:shadow-[inset_3px_0_0_var(--color-action)] focus-visible:outline-none",
        density === "compact" ? "py-1.5" : "py-2.5"
      )}
    >
      <PixelBot member={avatar.member} mode={avatar.mode} size={30} />
      <span className="min-w-0 flex-1">
        <span className="flex min-w-0 items-center gap-2">
          <span className="min-w-0 truncate font-bold text-t1">{task.title}</span>
        </span>
        <span className="mt-1 flex items-center gap-2 text-meta text-t3">
          <span className="font-mono">{taskCode(task.seq)}</span>
          {stage != null && stage < 5 && (
            <span className="flex items-center gap-[3px]" aria-label={`接力第 ${stage + 1} / 5 棒`}>
              {[0, 1, 2, 3, 4].map((i) => (
                <span
                  key={i}
                  className={cn(
                    "h-1.5 w-3.5 rounded-pill",
                    i < stage ? "bg-status-success" : i === stage ? (executing ? "animate-px-blink bg-status-running" : "bg-status-human") : "bg-line-strong"
                  )}
                />
              ))}
            </span>
          )}
          {task.currentRevision > 0 && <span className="font-pixel text-micro">R{task.currentRevision}</span>}
        </span>
      </span>
      <StateBadge status={task.status} size="sm" />
      <span className="inline-flex w-20 shrink-0 items-center justify-end gap-1 tabular-nums text-meta text-t3">
        <PixelIcon name="clock" size={9} />
        {relativeTime(task.updatedAt)}
      </span>
      <ChevronRight className="size-4 shrink-0 text-t3/0 transition-colors group-hover:text-t3" aria-hidden />
    </button>
  );
}
