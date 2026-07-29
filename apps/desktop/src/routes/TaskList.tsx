import { useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { ChevronRight, ShieldAlert } from "lucide-react";
import type { TaskSummary } from "@/generated/bindings";
import { useProjects } from "@/hooks/useProjects";
import { useTasks } from "@/hooks/useTasks";
import { useUiStore } from "@/stores/uiStore";
import { GROUP_LABEL, groupForStatus, type TaskGroup } from "@/copy/status";
import { relativeTime, taskCode } from "@/lib/format";
import { cn } from "@/lib/utils";
import { StateBadge } from "@/components/StateBadge";
import { AgentMark } from "@/components/AgentMark";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { Badge } from "@/components/ui/badge";
import { AuditExportDialog } from "@/components/AuditExportDialog";
import { PermissionProjectSummary } from "@/components/permission/PermissionProjectSummary";
import { ProjectOverview } from "@/components/home/ProjectOverview";
import { HomeEmpty } from "@/components/home/HomeEmpty";
import { AttentionCenter } from "@/components/home/AttentionCenter";
import { isTaskExecuting } from "@/lib/taskStatus";

export function TaskList() {
  const { projectId } = useParams();
  const navigate = useNavigate();
  const projects = useProjects();
  const tasks = useTasks(projectId);
  const openNewTask = useUiStore((s) => s.openNewTask);
  const [showDone, setShowDone] = useState(false);
  const [auditOpen, setAuditOpen] = useState(false);

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

  const counts = {
    total: tasks.data?.length ?? 0,
    attention: grouped.attention.length,
    active: grouped.active.length,
    done: grouped.done.length,
  };

  const onRow = (t: TaskSummary) => navigate(`/p/${projectId}/t/${t.id}`);
  const newTask = () => projectId && openNewTask(projectId);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto px-6 py-6">
        <div className="mx-auto flex max-w-4xl flex-col gap-7">
          {projectId && (
            <ProjectOverview
              project={project}
              counts={counts}
              onNew={newTask}
              onExport={() => setAuditOpen(true)}
              permissionSlot={<PermissionProjectSummary projectId={projectId} tasks={tasks.data} />}
              showStats={counts.total > 0}
            />
          )}

          {tasks.isLoading ? (
            <SkeletonRows rows={5} />
          ) : tasks.isError ? (
            <ErrorState error={tasks.error} onRetry={() => tasks.refetch()} />
          ) : counts.total === 0 ? (
            <HomeEmpty onNew={newTask} />
          ) : (
            <div className="flex flex-col gap-6">
              <AttentionCenter tasks={grouped.attention} onOpen={onRow} />
              <Group group="active" tasks={grouped.active} onRow={onRow} />
              {grouped.done.length > 0 && (
                <section>
                  <button
                    onClick={() => setShowDone((s) => !s)}
                    className="mb-2 flex items-center gap-1.5 px-1 text-[12px] font-semibold uppercase tracking-wider text-t3 transition-colors hover:text-t2"
                  >
                    <ChevronRight className={cn("size-3.5 transition-transform duration-200", showDone && "rotate-90")} />
                    {GROUP_LABEL.done}
                    <Badge>{grouped.done.length}</Badge>
                  </button>
                  <AnimatePresence initial={false}>
                    {showDone && (
                      <motion.div
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: "auto", opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        transition={{ duration: 0.24, ease: [0.16, 1, 0.3, 1] }}
                        className="overflow-hidden"
                      >
                        <div className="flex flex-col gap-1.5 pt-1">
                          {grouped.done.map((t, i) => (
                            <TaskRow key={t.id} task={t} onClick={() => onRow(t)} index={i} />
                          ))}
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </section>
              )}
            </div>
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

function Group({
  group,
  tasks,
  onRow,
}: {
  group: TaskGroup;
  tasks: TaskSummary[];
  onRow: (t: TaskSummary) => void;
}) {
  if (tasks.length === 0) return null;
  const attention = group === "attention";
  return (
    <section>
      <div
        className={cn(
          "mb-2.5 flex items-center gap-1.5 px-1 text-[12px] font-semibold uppercase tracking-wider",
          attention ? "text-human" : "text-t3"
        )}
      >
        {attention && <span className="size-1.5 animate-pulse-dot rounded-full bg-human shadow-[0_0_8px_-1px_var(--color-human)]" />}
        {GROUP_LABEL[group]}
        <Badge className={attention ? "bg-human-bg text-human" : ""}>{tasks.length}</Badge>
      </div>
      {tasks.length === 0 ? (
        <div className="rounded-[var(--radius-panel)] border border-dashed border-line/60 bg-panel/40 px-4 py-5 text-center text-[13px] text-t3">
          {attention ? "没有需要你处理的任务 ✦ 一切顺利" : "没有进行中的任务。"}
        </div>
      ) : (
        <div className="flex flex-col gap-1.5">
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
  const active = isTaskExecuting(task.status);
  return (
    <motion.button
      type="button"
      onClick={onClick}
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: Math.min(index * 0.035, 0.28), duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
      whileHover={{ y: -2 }}
      whileTap={{ scale: 0.995 }}
      className={cn(
        "group relative flex w-full items-center gap-3 overflow-hidden rounded-[12px] border px-3.5 py-2.5 text-left shadow-[0_1px_2px_rgba(90,68,42,0.04)] transition-[background,border-color,box-shadow]",
        attention
          ? "border-human/25 bg-human-bg/40 hover:border-human/45 hover:bg-human-bg/70 hover:shadow-[var(--shadow-raised)]"
          : "border-line/70 bg-panel/70 hover:border-line-strong hover:bg-raised hover:shadow-[var(--shadow-raised)]"
      )}
    >
      {/* 需要你：左侧一道橙色光带；其它：hover 时淡入的中性带 */}
      <span
        className={cn(
          "pointer-events-none absolute inset-y-0 left-0 w-[3px] rounded-r",
          attention ? "bg-human/70" : "bg-transparent group-hover:bg-line-strong"
        )}
        aria-hidden
      />
      <StateBadge status={task.status} size="sm" />
      <span className="shrink-0 font-mono text-[12px] text-t3">{taskCode(task.seq)}</span>
      <span className="min-w-0 flex-1 truncate font-medium text-t1 transition-transform group-hover:translate-x-0.5">
        {task.title}
      </span>
      {task.blockedReason === "permission_required" && (
        <span className="flex shrink-0 items-center gap-1 rounded-full border border-human/60 bg-human-bg px-2 py-0.5 text-[11px] font-medium text-human">
          <ShieldAlert className="size-3" aria-hidden /> 等待你授权
        </span>
      )}
      <span className="flex shrink-0 items-center gap-2 text-[12px]">
        {active && <AgentMark kind={task.developerAgent} size={22} />}
        {task.currentRevision > 0 && <span className="font-mono text-t3">r{task.currentRevision}</span>}
        <span className="tabular-nums text-t3">{relativeTime(task.updatedAt)}</span>
        <ChevronRight className="size-4 text-t3/0 transition-colors group-hover:text-t3" aria-hidden />
      </span>
    </motion.button>
  );
}
