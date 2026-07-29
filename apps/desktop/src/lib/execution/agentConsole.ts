import type { AgentKind, TaskDetail } from "@/generated/bindings";
import type { NodeState, Phase } from "@/copy/events";
import type { ExecutionTree, PhaseNode } from "./tree";

export type ConsolePhase = Extract<Phase, "plan" | "develop" | "validate" | "review">;

export interface AgentConsoleLane {
  phase: ConsolePhase;
  revision: number;
  label: string;
  state: NodeState;
  agents: AgentKind[];
  memberCount: number;
  fallbackCount: number;
  summary: string;
  runId: string | null;
}

const PHASES: Array<{ phase: ConsolePhase; label: string }> = [
  { phase: "plan", label: "计划" },
  { phase: "develop", label: "开发" },
  { phase: "validate", label: "验证" },
  { phase: "review", label: "审查" },
];

function defaultAgent(task: TaskDetail, phase: ConsolePhase): AgentKind[] {
  if (phase === "plan" || phase === "develop") return [task.developerAgent];
  if (phase === "review") return [task.reviewerAgent];
  return [];
}

function laneFromPhase(
  task: TaskDetail,
  config: (typeof PHASES)[number],
  revision: number,
  phase?: PhaseNode
): AgentConsoleLane {
  if (!phase) {
    const disabledPlan = config.phase === "plan" && !(task.policy.requirePlanApproval ?? false);
    return {
      ...config,
      revision,
      state: disabledPlan ? "info" : "pending",
      agents: disabledPlan ? [] : defaultAgent(task, config.phase),
      memberCount: disabledPlan ? 0 : config.phase === "validate" ? 1 : defaultAgent(task, config.phase).length,
      fallbackCount: 0,
      summary: disabledPlan ? "当前流程未启用" : "尚未开始",
      runId: null,
    };
  }
  const agents = [...new Set(phase.groups.map((group) => group.current.agent).filter((agent): agent is AgentKind => !!agent))];
  const fallbackCount = phase.groups.reduce(
    (sum, group) => sum + group.attempts.filter((attempt) => attempt.attemptLabel.startsWith("降级 ")).length,
    0
  );
  const current = phase.groups.find((group) => group.state === "running")?.current
    ?? phase.groups.find((group) => group.state === "attention" || group.state === "failed")?.current
    ?? phase.groups.at(-1)?.current;
  return {
    ...config,
    revision,
    state: phase.state,
    agents: agents.length > 0 ? agents : defaultAgent(task, config.phase),
    memberCount: Math.max(1, phase.groups.length),
    fallbackCount,
    summary: phase.summary ?? "已有执行记录",
    runId: current?.runId ?? null,
  };
}

export function buildAgentConsole(task: TaskDetail, tree: ExecutionTree): AgentConsoleLane[] {
  const current = tree.revisions.find((item) => item.revision === tree.currentRevision);
  return PHASES.map((config) => {
    const currentPhase = current?.phases.find((phase) => phase.phase === config.phase);
    // Planning is often recorded at r0, before the first code revision exists. Keep that
    // completed approval visible instead of claiming the current code revision never planned.
    const source = currentPhase || config.phase !== "plan"
      ? current
      : tree.revisions.find((item) => item.phases.some((phase) => phase.phase === "plan"));
    const phase = currentPhase ?? source?.phases.find((item) => item.phase === config.phase);
    return laneFromPhase(task, config, source?.revision ?? tree.currentRevision, phase);
  });
}
