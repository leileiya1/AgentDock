import type { TaskStatus } from "@/generated/bindings";

export type StatusTone =
  | "idle"
  | "run"
  | "review"
  | "human"
  | "ok"
  | "bad";

export interface StatusCopy {
  label: string;
  tone: StatusTone;
  /** in-progress states get a 1.5s breathing dot (reduced-motion → static). */
  pulse: boolean;
}

/**
 * Authoritative status → 文案 table (02 §8). Copy names things the user can
 * control/recognize; never leaks the raw enum.
 */
export const STATUS_COPY: Record<TaskStatus, StatusCopy> = {
  DRAFT: { label: "草稿", tone: "idle", pulse: false },
  PLANNING: { label: "拟定计划", tone: "run", pulse: true },
  WAITING_FOR_PLAN_APPROVAL: { label: "等你批计划", tone: "human", pulse: false },
  READY_FOR_DEVELOPMENT: { label: "排队开发", tone: "idle", pulse: false },
  DEVELOPING: { label: "开发中", tone: "run", pulse: true },
  VALIDATING: { label: "验证中", tone: "run", pulse: true },
  READY_FOR_REVIEW: { label: "排队审查", tone: "idle", pulse: false },
  REVIEWING: { label: "审查中", tone: "review", pulse: true },
  READY_FOR_REVISION: { label: "排队返工", tone: "idle", pulse: false },
  REVISING: { label: "返工中", tone: "run", pulse: true },
  WAITING_FOR_HUMAN_APPROVAL: { label: "等你批准", tone: "human", pulse: false },
  APPROVED: { label: "已批准 · 待合并", tone: "human", pulse: false },
  MERGING: { label: "合并中", tone: "run", pulse: true },
  MERGE_CONFLICT: { label: "合并冲突", tone: "human", pulse: false },
  MERGED: { label: "已合并", tone: "ok", pulse: false },
  ROLLED_BACK: { label: "已回滚", tone: "idle", pulse: false },
  BLOCKED: { label: "需要你处理", tone: "human", pulse: false },
  CANCELLED: { label: "已取消", tone: "idle", pulse: false },
};

/** Task-list grouping is product logic, not a sort preference (02 §4.2). */
export type TaskGroup = "attention" | "active" | "done";

const ATTENTION: TaskStatus[] = [
  "WAITING_FOR_HUMAN_APPROVAL",
  "WAITING_FOR_PLAN_APPROVAL",
  "BLOCKED",
  "MERGE_CONFLICT",
  "APPROVED",
];
const DONE: TaskStatus[] = ["MERGED", "ROLLED_BACK", "CANCELLED"];

export function groupForStatus(status: TaskStatus): TaskGroup {
  if (ATTENTION.includes(status)) return "attention";
  if (DONE.includes(status)) return "done";
  return "active";
}

export const GROUP_LABEL: Record<TaskGroup, string> = {
  attention: "需要你",
  active: "进行中",
  done: "已完结",
};
