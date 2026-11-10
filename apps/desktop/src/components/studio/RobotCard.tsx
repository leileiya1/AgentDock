import { forwardRef, useMemo } from "react";
import type { AgentKind, TaskSummary } from "@/generated/bindings";
import { PixelBot, type SquadMember } from "@/components/pixel/PixelBot";
import { useRuns } from "@/hooks/useTaskData";
import { useRunLog } from "@/hooks/useRunLog";
import { liveProgress } from "@/lib/logs/resultCard";
import { STATUS_COPY } from "@/copy/status";
import { agentLabel } from "@/copy/agents";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";

interface Props {
  member: string;
  name: string;
  /** 正在干嘛的一句话，比如「写代码」「喝茶」。 */
  doing: string;
  phrase?: string;
  color?: string | null;
  task?: TaskSummary;
  onOpenTask: (taskId: string) => void;
  onNewTask?: () => void;
  onClose: () => void;
}

/** 这个任务最近一次运行的最后一句人话（运行中就是实时的）。 */
function useLastLine(taskId: string | undefined) {
  const runs = useRuns(taskId);
  const run = useMemo(() => {
    const list = runs.data ?? [];
    return list.find((r) => r.status === "RUNNING") ?? [...list].sort((a, b) => (b.startedAt ?? "").localeCompare(a.startedAt ?? ""))[0];
  }, [runs.data]);
  const { buffer } = useRunLog(run?.id);
  const line = useMemo(() => (buffer ? liveProgress(buffer.lines, 1)[0]?.text ?? null : null), [buffer]);
  return { line, live: run?.status === "RUNNING" };
}

/**
 * 点小屋里的机器人弹出的名片：它是谁、手上是哪个任务、走到哪一步、
 * 最近一句输出。位置由画布每帧更新（跟着机器人走）。
 */
export const RobotCard = forwardRef<HTMLDivElement, Props>(function RobotCard({ member, name, doing, phrase, color, task, onOpenTask, onNewTask, onClose }, ref) {
  const { line, live } = useLastLine(task?.id);
  const status = task ? STATUS_COPY[task.status] : null;
  const isAgent = member !== "runner";
  return (
    <div
      ref={ref}
      role="dialog"
      aria-label={`${name} 的名片`}
      onPointerDown={(e) => e.stopPropagation()}
      className="pointer-events-auto absolute left-0 top-0 z-10 w-[260px] opacity-0"
    >
      <div className="overflow-hidden rounded-card bg-raised shadow-float ring-1 ring-line">
        <div className="flex items-center gap-2.5 bg-app px-3 py-2">
          <PixelBot member={member as SquadMember} mode={task ? "work" : "idle"} size={34} color={color ?? undefined} decorative />
          <div className="min-w-0 flex-1">
            <div className="truncate text-body font-extrabold text-t1">{name}</div>
            <div className="truncate text-micro font-semibold text-t3">{isAgent ? agentLabel(member as AgentKind) : "自动验证"} · {doing}</div>
          </div>
          <button type="button" onClick={onClose} aria-label="关闭名片" className="grid size-6 place-items-center rounded-row text-t3 hover:bg-raised hover:text-t1">
            ×
          </button>
        </div>
        <div className="flex flex-col gap-2 px-3 py-2.5">
          {phrase && <div className="text-meta italic text-t2">「{phrase}」</div>}
          {task ? (
            <>
              <div className="min-w-0">
                <div className="flex items-center gap-1.5">
                  <span className="font-mono text-micro text-t3">TASK-{String(task.seq).padStart(3, "0")}</span>
                  {status && (
                    <span className={cn("rounded-pill px-1.5 text-micro font-bold", status.tone === "human" ? "bg-status-human/10 text-status-human" : "bg-status-running/10 text-status-running")}>
                      {status.label}
                    </span>
                  )}
                  <span className="ml-auto text-micro text-t3">{relativeTime(task.updatedAt)}</span>
                </div>
                <div className="mt-0.5 line-clamp-2 text-meta font-bold text-t1">{task.title}</div>
              </div>
              <div className="rounded-row bg-studio-night px-2 py-1.5 font-mono text-micro leading-relaxed text-studio-day">
                <span className="mr-1 text-t3">{live ? "▶" : "■"}</span>
                {line ? <span className="line-clamp-2 inline">{line}</span> : <span className="text-t3">还没有输出</span>}
                {live && <span className="ml-0.5 animate-pulse">▍</span>}
              </div>
            </>
          ) : (
            <div className="text-meta text-t2">手上没活，正在{doing}。</div>
          )}
        </div>
        <div className="flex justify-end gap-2 border-t border-line px-3 py-2">
          {task ? (
            <button type="button" onClick={() => onOpenTask(task.id)} className="rounded-control bg-action px-3 py-1 text-meta font-bold text-white shadow-press-action hover:brightness-110">
              打开任务
            </button>
          ) : (
            onNewTask && (
              <button type="button" onClick={onNewTask} className="rounded-control bg-action px-3 py-1 text-meta font-bold text-white shadow-press-action hover:brightness-110">
                派个活
              </button>
            )
          )}
        </div>
      </div>
    </div>
  );
});
