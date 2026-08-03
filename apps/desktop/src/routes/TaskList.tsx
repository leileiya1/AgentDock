import { useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ChevronRight, ListFilter, Search } from "lucide-react";
import type { TaskSummary } from "@/generated/bindings";
import { useProjects } from "@/hooks/useProjects";
import { useTasks } from "@/hooks/useTasks";
import { useUiStore } from "@/stores/uiStore";
import { relativeTime, taskCode } from "@/lib/format";
import { buildHomeTaskSections, type HomeView } from "@/lib/homeTasks";
import { cn } from "@/lib/utils";
import { StateBadge } from "@/components/StateBadge";
import { AgentMark } from "@/components/AgentMark";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { Badge } from "@/components/ui/badge";
import { AuditExportDialog } from "@/components/AuditExportDialog";
import { PermissionProjectSummary } from "@/components/permission/PermissionProjectSummary";
import { ProjectOverview } from "@/components/home/ProjectOverview";
import { ProjectEnvironmentStatus } from "@/components/home/ProjectEnvironmentStatus";
import { HomeEmpty } from "@/components/home/HomeEmpty";
import { AttentionCenter } from "@/components/home/AttentionCenter";
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
  const openNewTask = useUiStore((s) => s.openNewTask);
  const [homeView, setHomeView] = useState<HomeView>("attention");
  const [showDone, setShowDone] = useState(false);
  const [auditOpen, setAuditOpen] = useState(false);
  const [query, setQuery] = useState("");
  const density = useUiStore((s) => s.densityMode);
  const toggleDensity = useUiStore((s) => s.toggleDensity);

  const project = projects.data?.find((item) => item.id === projectId);
  const filteredTasks = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) return tasks.data ?? [];
    return (tasks.data ?? []).filter((task) => [
      taskCode(task.seq), task.title, agentLabel(task.developerAgent), agentLabel(task.reviewerAgent), STATUS_COPY[task.status].label,
    ].some((value) => value.toLocaleLowerCase().includes(normalized)));
  }, [tasks.data, query]);
  const sections = useMemo(() => buildHomeTaskSections(filteredTasks), [filteredTasks]);

  const onRow = (task: TaskSummary) => navigate(`/p/${projectId}/t/${task.id}`);
  const newTask = () => projectId && openNewTask(projectId);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto px-6 py-6">
        <div className="mx-auto flex max-w-4xl flex-col gap-7">
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
              showStats={sections.counts.all > 0}
            />
          )}

          {(tasks.data?.length ?? 0) > 0 && (
            <div className="flex items-center gap-2" role="search">
              <div className="relative min-w-0 flex-1">
                <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-t3" aria-hidden />
                <Input
                  id="task-search"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  className="pl-9"
                  aria-label="搜索任务"
                  placeholder="搜索 TASK、标题、Provider 或状态"
                />
              </div>
              <Button variant="outline" onClick={toggleDensity} title="切换任务列表、执行轨道和日志的行距">
                <ListFilter className="size-4" /> {density === "compact" ? "舒适" : "紧凑"}
              </Button>
            </div>
          )}

          {tasks.isLoading ? (
            <SkeletonRows rows={5} />
          ) : tasks.isError ? (
            <ErrorState error={tasks.error} onRetry={() => tasks.refetch()} />
          ) : sections.counts.all === 0 ? (
            <HomeEmpty onNew={newTask} />
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
            <div className="flex flex-col gap-1.5 pt-1">
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
      <div className="mb-2.5 flex items-center gap-1.5 px-1 text-meta font-semibold uppercase tracking-wider text-t3">
        {title}
        <Badge>{tasks.length}</Badge>
      </div>
      {tasks.length === 0 ? (
        <div className="rounded-section border border-dashed border-line/60 bg-panel/40 px-4 py-5 text-center text-body text-t3">
          {emptyText}
        </div>
      ) : (
        <div className="flex flex-col gap-1.5">
          {tasks.map((task) => (
            <TaskRow key={task.id} task={task} onClick={() => onRow(task)} density={density} />
          ))}
        </div>
      )}
    </section>
  );
}

function TaskRow({ task, onClick, density }: { task: TaskSummary; onClick: () => void; density: "comfortable" | "compact" }) {
  const active = isTaskExecuting(task.status);
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn("group relative flex w-full items-center gap-3 overflow-hidden rounded-row border border-line/70 bg-panel/70 px-3.5 text-left transition-colors hover:border-line-strong hover:bg-raised", density === "compact" ? "py-1.5" : "py-2.5")}
    >
      <span className="pointer-events-none absolute inset-y-0 left-0 w-[3px] rounded-r bg-transparent group-hover:bg-line-strong" aria-hidden />
      <StateBadge status={task.status} size="sm" />
      <span className="shrink-0 font-mono text-meta text-t3">{taskCode(task.seq)}</span>
      <span className="min-w-0 flex-1 truncate font-medium text-t1">
        {task.title}
      </span>
      <span className="flex shrink-0 items-center gap-2 text-meta">
        {active && <AgentMark kind={task.developerAgent} size={22} />}
        {task.currentRevision > 0 && <span className="font-mono text-t3">r{task.currentRevision}</span>}
        <span className="tabular-nums text-t3">{relativeTime(task.updatedAt)}</span>
        <ChevronRight className="size-4 text-t3/0 transition-colors group-hover:text-t3" aria-hidden />
      </span>
    </button>
  );
}
