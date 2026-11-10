import type { TaskStatus, TaskSummary } from "@/generated/bindings";
import { taskCode } from "@/lib/format";

/** 成员在小屋里的活动。只由后端任务状态推导，不推测进度。 */
export type Activity = "plan" | "develop" | "validate" | "review" | "waiting" | "idle";

export interface MemberActivity {
  /** Provider id、"runner"（本地验证机）。 */
  id: string;
  activity: Activity;
  /** 正在处理 / 等你处理的任务编号；空闲时为 null。 */
  taskCode: string | null;
  /** 对应任务 id，用于点击机器人打开任务。 */
  taskId: string | null;
}

type Who = "developer" | "reviewer" | "runner";

const RULES: Partial<Record<TaskStatus, { who: Who; activity: Activity }>> = {
  PLANNING: { who: "developer", activity: "plan" },
  DEVELOPING: { who: "developer", activity: "develop" },
  REVISING: { who: "developer", activity: "develop" },
  VALIDATING: { who: "runner", activity: "validate" },
  MERGING: { who: "runner", activity: "validate" },
  REVIEWING: { who: "reviewer", activity: "review" },
  WAITING_FOR_PLAN_APPROVAL: { who: "developer", activity: "waiting" },
  BLOCKED: { who: "developer", activity: "waiting" },
  WAITING_FOR_HUMAN_APPROVAL: { who: "reviewer", activity: "waiting" },
  MERGE_CONFLICT: { who: "reviewer", activity: "waiting" },
};

/** 等你 > 工作中 > 空闲；工作中之间不分高低，先到先得（按任务更新时间倒序）。 */
const RANK: Record<Activity, number> = { waiting: 3, plan: 2, develop: 2, validate: 2, review: 2, idle: 0 };

/**
 * 每个成员当前最需要被看见的一个活动。成员名单 = 任务里出现过的开发 / 审查 Agent + 验证机，
 * 顺序稳定：先开发者、再审查者、最后验证机（与小队条一致）。
 */
export function memberActivities(tasks: readonly TaskSummary[]): MemberActivity[] {
  const order: string[] = [];
  const push = (id: string) => {
    if (!order.includes(id)) order.push(id);
  };
  tasks.forEach((task) => push(task.developerAgent));
  tasks.forEach((task) => push(task.reviewerAgent));
  push("runner");

  const best = new Map<string, MemberActivity>();
  const recent = [...tasks].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  for (const task of recent) {
    const rule = RULES[task.status];
    if (!rule) continue;
    const id = rule.who === "developer" ? task.developerAgent : rule.who === "reviewer" ? task.reviewerAgent : "runner";
    const current = best.get(id);
    if (!current || RANK[rule.activity] > RANK[current.activity]) {
      best.set(id, { id, activity: rule.activity, taskCode: taskCode(task.seq), taskId: task.id });
    }
  }
  return order.map((id) => best.get(id) ?? { id, activity: "idle", taskCode: null, taskId: null });
}
