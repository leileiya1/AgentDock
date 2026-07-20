import { useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { motion } from "motion/react";
import { ChevronRight, Plus } from "lucide-react";
import type { TaskSummary } from "@/generated/bindings";
import { useProjects } from "@/hooks/useProjects";
import { useTasks } from "@/hooks/useTasks";
import { useUiStore } from "@/stores/uiStore";
import { GROUP_LABEL, groupForStatus, type TaskGroup } from "@/copy/status";
import { relativeTime, taskCode } from "@/lib/format";
import { cn } from "@/lib/utils";
import { StateBadge } from "@/components/StateBadge";
import { AgentMark } from "@/components/AgentMark";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";

const ACTIVE_STATUSES = new Set(["DEVELOPING", "VALIDATING", "REVIEWING", "REVISING", "MERGING"]);

export function TaskList() {
  const { projectId } = useParams();
  const navigate = useNavigate();
  const projects = useProjects();
  const tasks = useTasks(projectId);
  const openNewTask = useUiStore((s) => s.openNewTask);
  const [showDone, setShowDone] = useState(false);

  const project = projects.data?.find((p) => p.id === projectId);

  const grouped = useMemo(() => {
    const g: Record<TaskGroup, TaskSummary[]> = { attention: [], active: [], done: [] };
    for (const t of tasks.data ?? []) g[groupForStatus(t.status)].push(t);
    const byTime = (a: TaskSummary, b: TaskSummary) =>
      new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime();
    g.attention.sort(byTime);
    g.active.sort(byTime);
    g.done.sort(byTime);
    return g;
  }, [tasks.data]);

  const onRow = (t: TaskSummary) => navigate(`/p/${projectId}/t/${t.id}`);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex shrink-0 items-center justify-between gap-4 border-b border-line/70 px-6 py-4">
        <div className="flex min-w-0 items-baseline gap-3">
          <h1 className="text-xl font-semibold tracking-tight">{project?.name ?? "项目"}</h1>
          {project && (
            <span className="font-mono text-[12px] text-t3">分支 {project.defaultBranch}</span>
          )}
        </div>
        <Button variant="primary" onClick={() => projectId && openNewTask(projectId)} title="新建任务 (⌘N)">
          <Plus className="size-4" /> 新建任务
        </Button>
      </header>

      <div className="flex-1 overflow-y-auto px-6 py-5">
        {tasks.isLoading ? (
          <div className="max-w-3xl">
            <SkeletonRows rows={5} />
          </div>
        ) : tasks.isError ? (
          <ErrorState error={tasks.error} onRetry={() => tasks.refetch()} />
        ) : (tasks.data?.length ?? 0) === 0 ? (
          <EmptyState
            title="还没有任务"
            hint="新建一个，写清楚你想要什么——描述会原文交给开发 Agent。"
            action={
              <Button variant="primary" onClick={() => projectId && openNewTask(projectId)}>
                <Plus className="size-4" /> 新建任务
              </Button>
            }
          />
        ) : (
          <div className="mx-auto flex max-w-3xl flex-col gap-6">
            <Group group="attention" tasks={grouped.attention} onRow={onRow} alwaysShow />
            <Group group="active" tasks={grouped.active} onRow={onRow} />
            {grouped.done.length > 0 && (
              <section>
                <button
                  onClick={() => setShowDone((s) => !s)}
                  className="mb-2 flex items-center gap-1.5 px-1 text-[12px] font-semibold uppercase tracking-wider text-t3 transition-colors hover:text-t2"
                >
                  <ChevronRight className={cn("size-3.5 transition-transform", showDone && "rotate-90")} />
                  {GROUP_LABEL.done}
                  <Badge>{grouped.done.length}</Badge>
                </button>
                {showDone &&
                  grouped.done.map((t) => <TaskRow key={t.id} task={t} onClick={() => onRow(t)} />)}
              </section>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function Group({
  group,
  tasks,
  onRow,
  alwaysShow,
}: {
  group: TaskGroup;
  tasks: TaskSummary[];
  onRow: (t: TaskSummary) => void;
  alwaysShow?: boolean;
}) {
  if (tasks.length === 0 && !alwaysShow) return null;
  const attention = group === "attention";
  return (
    <section>
      <div
        className={cn(
          "mb-2 flex items-center gap-1.5 px-1 text-[12px] font-semibold uppercase tracking-wider",
          attention ? "text-human" : "text-t3"
        )}
      >
        {attention && <span className="size-1.5 rounded-full bg-human shadow-[0_0_8px_-1px_var(--color-human)]" />}
        {GROUP_LABEL[group]}
        <Badge className={attention ? "bg-human-bg text-human" : ""}>{tasks.length}</Badge>
      </div>
      {tasks.length === 0 ? (
        <div className="rounded-[var(--radius-panel)] border border-dashed border-line/60 px-3 py-4 text-[13px] text-t3">
          {attention ? "没有需要你处理的任务。" : "没有进行中的任务。"}
        </div>
      ) : (
        <div className="flex flex-col gap-1">
          {tasks.map((t, i) => (
            <TaskRow key={t.id} task={t} onClick={() => onRow(t)} index={i} attention={attention} />
          ))}
        </div>
      )}
    </section>
  );
}

function TaskRow({
  task,
  onClick,
  index = 0,
  attention,
}: {
  task: TaskSummary;
  onClick: () => void;
  index?: number;
  attention?: boolean;
}) {
  const active = ACTIVE_STATUSES.has(task.status);
  return (
    <motion.button
      type="button"
      onClick={onClick}
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: Math.min(index * 0.02, 0.2), duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
      whileHover={{ x: 2 }}
      className={cn(
        "group flex h-10 w-full items-center gap-3 rounded-[var(--radius-control)] border px-3 text-left transition-colors",
        attention
          ? "border-human/20 bg-human-bg/30 hover:border-human/40 hover:bg-human-bg/60"
          : "border-transparent hover:border-line hover:bg-panel"
      )}
    >
      <StateBadge status={task.status} size="sm" />
      <span className="shrink-0 font-mono text-[12px] text-t3">{taskCode(task.seq)}</span>
      <span className="min-w-0 flex-1 truncate text-t1">{task.title}</span>
      <span className="flex shrink-0 items-center gap-2 text-[12px]">
        {active && <AgentMark kind={task.developerAgent} />}
        {task.currentRevision > 0 && <span className="font-mono text-t3">r{task.currentRevision}</span>}
        <span className="text-t3">{relativeTime(task.updatedAt)}</span>
      </span>
    </motion.button>
  );
}
