import type { TaskStatus, TaskSummary } from "@/generated/bindings";
import type { BotMode, SquadMember } from "@/components/pixel/PixelBot";
import { groupForStatus } from "@/copy/status";
import { taskCode } from "@/lib/format";

/**
 * The relay has five stations: 计划 → 开发 → 验证 → 审查 → 你。
 * These helpers only translate backend task status into who holds the baton;
 * they never invent progress the backend has not confirmed.
 */
export const RELAY_STATIONS = ["计划", "开发", "验证", "审查", "拍板"] as const;

const STAGE: Partial<Record<TaskStatus, number>> = {
  DRAFT: 0,
  PLANNING: 0,
  WAITING_FOR_PLAN_APPROVAL: 0,
  READY_FOR_DEVELOPMENT: 1,
  DEVELOPING: 1,
  READY_FOR_REVISION: 1,
  REVISING: 1,
  VALIDATING: 2,
  READY_FOR_REVIEW: 3,
  REVIEWING: 3,
  WAITING_FOR_HUMAN_APPROVAL: 4,
  APPROVED: 4,
  MERGING: 4,
  MERGE_CONFLICT: 4,
  MERGED: 5,
};

/** Index of the station currently holding the baton; 5 means every leg is done. */
export function relayStage(task: Pick<TaskSummary, "status">): number | null {
  return STAGE[task.status] ?? null;
}

const ACTIVE: ReadonlySet<TaskStatus> = new Set(["PLANNING", "DEVELOPING", "REVISING", "VALIDATING", "REVIEWING", "MERGING"]);

/** Who is visibly working on a task right now, if anyone. */
export function actingMember(task: Pick<TaskSummary, "status" | "developerAgent" | "reviewerAgent">): SquadMember | null {
  if (!ACTIVE.has(task.status)) return null;
  if (task.status === "VALIDATING") return "runner";
  if (task.status === "REVIEWING") return task.reviewerAgent;
  if (task.status === "MERGING") return "runner";
  return task.developerAgent;
}

/** The robot that represents a task in a list row, with the pose that matches its state. */
export function taskAvatar(task: Pick<TaskSummary, "status" | "developerAgent" | "reviewerAgent">): { member: SquadMember; mode: BotMode } {
  const acting = actingMember(task);
  if (acting) return { member: acting, mode: "work" };
  switch (task.status) {
    case "WAITING_FOR_PLAN_APPROVAL":
    case "BLOCKED":
      return { member: task.developerAgent, mode: "hand" };
    case "WAITING_FOR_HUMAN_APPROVAL":
    case "APPROVED":
      return { member: task.reviewerAgent, mode: "done" };
    case "MERGE_CONFLICT":
      return { member: task.reviewerAgent, mode: "hand" };
    case "MERGED":
      return { member: "you", mode: "done" };
    default:
      return { member: task.developerAgent, mode: "idle" };
  }
}

export interface SquadSlot {
  member: SquadMember;
  mode: BotMode;
  /** Task code the member is working on, shown as a speech bubble. */
  working: string | null;
}

/**
 * Members seen in this project's tasks, plus the local validator. Order is stable:
 * developers first, then reviewers, then the validator.
 */
export function buildSquad(tasks: TaskSummary[], limit = 5): { slots: SquadSlot[]; waitingForYou: number } {
  const order: SquadMember[] = [];
  const push = (member: SquadMember) => {
    if (!order.includes(member)) order.push(member);
  };
  tasks.forEach((task) => push(task.developerAgent));
  tasks.forEach((task) => push(task.reviewerAgent));
  push("runner");

  const working = new Map<SquadMember, string>();
  for (const task of tasks) {
    const member = actingMember(task);
    if (member && !working.has(member)) working.set(member, taskCode(task.seq));
  }

  const slots = order.slice(0, limit).map((member) => ({
    member,
    mode: working.has(member) ? ("work" as const) : ("idle" as const),
    working: working.get(member) ?? null,
  }));
  const waitingForYou = tasks.filter((task) => groupForStatus(task.status) === "attention").length;
  return { slots, waitingForYou };
}
