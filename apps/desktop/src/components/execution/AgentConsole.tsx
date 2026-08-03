import { ChevronRight, RotateCcw, Users } from "lucide-react";
import type { TaskDetail } from "@/generated/bindings";
import type { ExecutionTree } from "@/lib/execution/tree";
import { agentLabel } from "@/copy/agents";
import { buildAgentConsole, type ConsolePhase } from "@/lib/execution/agentConsole";
import { AgentMark } from "@/components/AgentMark";
import { cn } from "@/lib/utils";
import { PhaseIcon, StateMark } from "./StateMark";
import { useUiStore } from "@/stores/uiStore";

export function AgentConsole({
  task,
  tree,
  onOpen,
}: {
  task: TaskDetail;
  tree: ExecutionTree;
  onOpen: (phase: ConsolePhase, runId: string | null, revision: number) => void;
}) {
  const density = useUiStore((state) => state.densityMode);
  const lanes = buildAgentConsole(task, tree);
  const active = lanes.filter((lane) => lane.state === "running").length;
  const participants = new Set(lanes.flatMap((lane) => lane.agents));

  return (
    <section className="rounded-section border border-line/80 bg-panel/70 p-3" aria-label="执行轨道">
      <div className="mb-2.5 flex flex-wrap items-center gap-2">
        <h2 className="text-body font-semibold text-t1">执行轨道</h2>
        <span className="text-meta text-t3">{task.currentRevision > 0 ? `r${task.currentRevision}` : "准备阶段"}</span>
        <span className="ml-auto flex items-center gap-1.5 text-meta text-t3">
          <Users className="size-3.5" aria-hidden />
          {participants.size} 个 Provider
          {active > 0 && <span className="text-status-running">· {active} 个阶段运行中</span>}
        </span>
      </div>

      <div className="grid grid-cols-2 gap-2 xl:grid-cols-4">
        {lanes.map((lane) => (
          <button
            key={lane.phase}
            type="button"
            onClick={() => onOpen(lane.phase, lane.runId, lane.revision)}
            className={cn(
              "group min-w-0 rounded-section border text-left transition-colors",
              density === "compact" ? "p-1.5" : "p-2.5",
              lane.state === "running"
                ? "border-status-running/35 bg-status-running/5 hover:border-status-running/60"
                : lane.state === "failed"
                  ? "border-status-danger/35 bg-status-human-bg/30 hover:border-status-danger/60"
                  : lane.state === "attention"
                    ? "border-status-human/35 bg-status-human-bg/35 hover:border-status-human/60"
                    : "border-line/70 bg-app/50 hover:border-line-strong hover:bg-raised/60"
            )}
            aria-label={`${lane.label}：${lane.summary}`}
          >
            <span className="flex items-center gap-2">
              <span className="grid size-7 shrink-0 place-items-center rounded-control border border-line/70 bg-panel">
                <PhaseIcon phase={lane.phase} className="size-3.5" />
              </span>
              <span className="min-w-0 flex-1 text-meta font-semibold text-t1">{lane.label}</span>
              <StateMark state={lane.state} iconOnly />
            </span>

            <span className="mt-2 flex min-h-6 items-center gap-1.5">
              {lane.phase === "plan" && lane.memberCount === 0 ? (
                <span className="rounded-pill border border-line px-2 py-0.5 text-meta text-t3">无需计划审批</span>
              ) : lane.phase === "validate" && lane.agents.length === 0 ? (
                <span className="rounded-pill border border-line px-2 py-0.5 text-meta text-t2">本机验证</span>
              ) : (
                <>
                  <span className="flex -space-x-1.5">
                    {lane.agents.slice(0, 3).map((agent) => (
                      <span key={agent} className="rounded-circle bg-panel ring-2 ring-panel">
                        <AgentMark kind={agent} size={22} title={agentLabel(agent)} />
                      </span>
                    ))}
                  </span>
                  <span className="min-w-0 truncate text-meta text-t2">
                    {lane.agents.map(agentLabel).join("、") || "等待分配"}
                  </span>
                </>
              )}
            </span>

            <span className="mt-1.5 flex items-center gap-1 text-meta text-t3">
              <span className="min-w-0 flex-1 truncate">{lane.summary}</span>
              {lane.memberCount > 1 && <span className="shrink-0">并行 {lane.memberCount}</span>}
              {lane.fallbackCount > 0 && (
                <span className="flex shrink-0 items-center gap-0.5 text-caution">
                  <RotateCcw className="size-3" aria-hidden /> 降级 {lane.fallbackCount}
                </span>
              )}
              <ChevronRight className="size-3.5 shrink-0 opacity-0 transition-opacity group-hover:opacity-100" aria-hidden />
            </span>
          </button>
        ))}
      </div>
    </section>
  );
}
