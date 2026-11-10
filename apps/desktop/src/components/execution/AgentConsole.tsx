import { RotateCcw } from "lucide-react";
import type { TaskDetail } from "@/generated/bindings";
import type { NodeState } from "@/copy/events";
import type { ExecutionTree } from "@/lib/execution/tree";
import { groupForStatus } from "@/copy/status";
import { buildAgentConsole, type ConsolePhase } from "@/lib/execution/agentConsole";
import { PixelBot, botTint, memberLabel, type BotMode, type SquadMember } from "@/components/pixel/PixelBot";
import { PixelIcon } from "@/components/pixel/PixelIcon";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/uiStore";

/**
 * 接力轨道：计划 → 开发 → 验证 → 审查 → 你。每个工位是一个像素队员，
 * 只有真正在跑的阶段会动；状态全部来自执行树，前端不推测进度。
 */
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
  const runningIndex = lanes.findIndex((lane) => lane.state === "running");
  // When a lane itself is waiting (e.g. plan approval) that lane raises its hand;
  // the final "你" station only takes the baton for the final decision.
  const laneWaiting = lanes.some((lane) => lane.state === "attention" || lane.state === "failed");
  const humanTurn = groupForStatus(task.status) === "attention" && !laneWaiting;
  const merged = task.status === "MERGED";
  const waitingIndex = lanes.findIndex((lane) => lane.state === "attention" || lane.state === "failed");
  const step = merged || humanTurn ? 5 : runningIndex >= 0 ? runningIndex + 1 : waitingIndex >= 0 ? waitingIndex + 1 : null;

  return (
    <section className="rounded-card border border-line bg-raised px-4 pb-4 pt-3" aria-label="接力轨道">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="text-section font-bold text-t1">接力</h2>
        <span className="font-pixel text-micro text-t3">
          {task.currentRevision > 0 ? `R${task.currentRevision}` : "PREP"}
          {step != null && ` · STEP ${step}/5`}
        </span>
      </div>

      <ol className={cn("flex items-stretch", density === "compact" ? "pt-5" : "pt-6")} aria-label="接力工位">
        {lanes.map((lane, index) => {
          const member: SquadMember = lane.phase === "validate" ? (lane.agents[0] ?? "runner") : (lane.agents[0] ?? task.developerAgent);
          const prevDone = index > 0 && isDone(lanes[index - 1].state);
          const linkRunning = index > 0 && lanes[index - 1].state === "running";
          return (
            <li key={lane.phase} className={cn("flex min-w-0", lane.state === "running" ? "flex-[1.3_1_0]" : "flex-1")}>
              {index > 0 && <Link done={prevDone} running={linkRunning} />}
              <Station
                label={lane.label}
                member={member}
                mode={modeFor(lane.state)}
                state={lane.state}
                sub={lane.state === "info" && lane.phase === "plan" ? "未启用" : lane.agents.length > 1 ? `${memberLabel(member)} 等 ${lane.agents.length} 位` : memberLabel(member)}
                bubble={lane.state === "running" ? PHASE_VERB[lane.phase] : lane.state === "attention" ? "等你处理" : lane.state === "failed" ? "没通过" : null}
                fallbackCount={lane.fallbackCount}
                parallel={lane.memberCount > 1 ? lane.memberCount : 0}
                compact={density === "compact"}
                onClick={() => onOpen(lane.phase, lane.runId, lane.revision)}
              />
            </li>
          );
        })}
        <li className={cn("flex min-w-0", humanTurn ? "flex-[1.3_1_0]" : "flex-1")}>
          <Link done={merged || humanTurn} running={false} />
          <Station
            label="拍板"
            member="you"
            mode={merged ? "done" : humanTurn ? "hand" : "idle"}
            state={merged ? "ok" : humanTurn ? "attention" : "pending"}
            sub={merged ? "已合并" : "你"}
            bubble={humanTurn ? "轮到你了" : null}
            fallbackCount={0}
            parallel={0}
            compact={density === "compact"}
          />
        </li>
      </ol>
    </section>
  );
}

function isDone(state: NodeState): boolean {
  return state === "ok" || state === "info";
}

function modeFor(state: NodeState): BotMode {
  if (state === "running") return "work";
  if (state === "ok") return "done";
  if (state === "attention" || state === "failed") return "hand";
  return "idle";
}

function Link({ done, running }: { done: boolean; running: boolean }) {
  return (
    <span
      aria-hidden
      className={cn(
        "-mt-5 h-1 w-6 shrink-0 self-center rounded-pill",
        done ? "bg-status-success" : running ? "relay-march animate-relay-march" : "relay-wait"
      )}
    />
  );
}

function Station({
  label,
  member,
  mode,
  state,
  sub,
  bubble,
  fallbackCount,
  parallel,
  compact,
  onClick,
}: {
  label: string;
  member: SquadMember;
  mode: BotMode;
  state: NodeState;
  sub: string;
  bubble: string | null;
  fallbackCount: number;
  parallel: number;
  compact: boolean;
  onClick?: () => void;
}) {
  const waiting = state === "pending" || (state === "info" && mode === "idle" && sub === "未启用");
  const active = state === "running" || state === "attention" || state === "failed";
  const human = state === "attention" || state === "failed";
  const Tag = onClick ? "button" : "div";
  return (
    <Tag
      {...(onClick ? { type: "button" as const, onClick } : {})}
      aria-label={`${label}：${memberLabel(member)}，${STATE_LABEL[state]}`}
      className={cn(
        "relative flex min-w-0 flex-1 flex-col items-center gap-1 rounded-section text-center transition-[background,box-shadow] duration-300",
        compact ? "px-1.5 pb-2 pt-3" : "px-2 pb-3 pt-4",
        onClick && "hover:brightness-[0.98]",
        active && (human ? "ring-2 ring-status-human/60" : "ring-2 ring-status-running")
      )}
      style={{ background: waiting ? "var(--color-station-wait)" : botTint(member) }}
    >
      {bubble && (
        <span
          className={cn(
            "absolute -top-3.5 left-1/2 flex max-w-[calc(100%+24px)] -translate-x-1/2 animate-pop-in items-center gap-1.5 truncate whitespace-nowrap rounded-control px-2.5 py-1 text-micro font-bold text-white",
            human ? "bg-status-human" : "bg-status-running"
          )}
        >
          <span className="truncate">{bubble}</span>
          {!human && (
            <span className="flex gap-0.5" aria-hidden>
              <span className="size-[3px] animate-px-blink bg-white" />
              <span className="size-[3px] animate-px-blink bg-white [animation-delay:.33s]" />
              <span className="size-[3px] animate-px-blink bg-white [animation-delay:.66s]" />
            </span>
          )}
        </span>
      )}
      {state === "ok" && (
        <span className="absolute right-2 top-2 grid size-4 place-items-center rounded-row bg-status-success text-white" aria-hidden>
          <PixelIcon name="check" size={8} />
        </span>
      )}
      {state === "pending" && (
        <span className="absolute right-2 top-2 grid size-4 place-items-center rounded-row bg-raised text-t3" aria-hidden>
          <PixelIcon name="hourglass" size={8} />
        </span>
      )}
      <PixelBot member={member} mode={mode} size={active ? (compact ? 40 : 48) : compact ? 32 : 40} decorative />
      <span
        className={cn(
          "text-body font-bold",
          state === "running" ? "text-status-running" : human ? "text-status-human" : waiting ? "text-t3" : "text-t1"
        )}
      >
        {label}
      </span>
      <span className="flex max-w-full items-center gap-1 truncate text-micro text-t3">
        <span className="truncate">{sub}</span>
        {parallel > 0 && <span className="shrink-0">· 并行 {parallel}</span>}
        {fallbackCount > 0 && (
          <span className="flex shrink-0 items-center gap-0.5 text-caution" title={`降级 ${fallbackCount} 次`}>
            <RotateCcw className="size-3" aria-hidden />
            {fallbackCount}
          </span>
        )}
      </span>
    </Tag>
  );
}

const PHASE_VERB: Record<ConsolePhase, string> = {
  plan: "拆步骤",
  develop: "写代码",
  validate: "跑验证",
  review: "读改动",
};

const STATE_LABEL: Record<NodeState, string> = {
  pending: "等待接棒",
  running: "进行中",
  ok: "已交棒",
  attention: "需要你处理",
  failed: "未通过",
  info: "未启用",
};
