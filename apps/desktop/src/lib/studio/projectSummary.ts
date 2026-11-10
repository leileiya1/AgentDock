import type { AgentKind, ProjectSummary, TaskSummary } from "@/generated/bindings";
import { groupForStatus } from "@/copy/status";

const EXECUTING_DEV = new Set(["PLANNING", "DEVELOPING", "REVISING"]);

/**
 * 与后端 `summarize_projects` 相同的规则，只用于浏览器预览（dev shim）和对拍测试；
 * 真实数据一律来自后端 `project_summaries` 命令。
 */
export function summarizeTasks(projectId: string, tasks: readonly TaskSummary[], recentCutoff: string, failingTaskIds: ReadonlySet<string> = new Set()): ProjectSummary {
  const own = tasks.filter((t) => t.projectId === projectId).sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  const agents: AgentKind[] = [];
  const active: AgentKind[] = [];
  const push = (list: AgentKind[], agent: AgentKind) => {
    if (!list.includes(agent)) list.push(agent);
  };
  let running = 0, waiting = 0, done = 0, failing = 0;
  for (const t of own) {
    const group = groupForStatus(t.status);
    if (group === "attention") waiting++;
    else if (group === "done") done++;
    else running++;
    if (failingTaskIds.has(t.id) && group !== "done") failing++;
    if (t.updatedAt >= recentCutoff) {
      push(agents, t.developerAgent);
      push(agents, t.reviewerAgent);
    }
    if (EXECUTING_DEV.has(t.status)) push(active, t.developerAgent);
    else if (t.status === "REVIEWING") push(active, t.reviewerAgent);
  }
  return {
    projectId,
    taskCount: own.length,
    running,
    waitingForYou: waiting,
    done,
    failing,
    agents,
    activeAgents: active,
    lastActivityAt: own[0]?.updatedAt ?? null,
    assignedAgents: [],
    agentsSource: "derived",
  };
}

/** 与后端 `apply_members` 相同：有分配成员就用成员（assigned），否则保留任务推导（derived）。 */
export function withMembers(summary: ProjectSummary, members: readonly AgentKind[]): ProjectSummary {
  return members.length > 0
    ? { ...summary, assignedAgents: [...members], agentsSource: "assigned" }
    : { ...summary, assignedAgents: [], agentsSource: "derived" };
}

/** 小镇里这个项目住几位 Agent：用户分配了就按分配的来，没分配过才按最近任务推导。 */
export function townAgents(summary: ProjectSummary | undefined): AgentKind[] {
  if (!summary) return [];
  return summary.agentsSource === "assigned" ? summary.assignedAgents : summary.agents;
}
