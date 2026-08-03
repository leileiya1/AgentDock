import type { BlockedReason, TaskSummary } from "@/generated/bindings";
import { BLOCKED_COPY } from "@/copy/blocked";

export type AttentionKind = "permission" | "approval" | "recovery" | "conflict" | "delivery";
export type AttentionSeverity = "high" | "medium" | "normal";

export interface AttentionItem {
  task: TaskSummary;
  kind: AttentionKind;
  severity: AttentionSeverity;
  reason: string;
  action: string;
  groupKey: string;
}

export interface AttentionGroup {
  key: string;
  label: string;
  items: AttentionItem[];
}

const SEVERITY_RANK: Record<AttentionSeverity, number> = { high: 0, medium: 1, normal: 2 };

const HIGH_IMPACT = new Set<BlockedReason>([
  "review_block",
  "quality_regressed",
  "recovery_failed",
  "worktree_missing",
  "commit_guard",
  "quality_gate",
]);

const NORMAL_DECISION = new Set<BlockedReason>([
  "permission_required",
  "needs_clarification",
  "no_changes",
]);

function blockedSeverity(reason: BlockedReason | null): AttentionSeverity {
  if (reason && HIGH_IMPACT.has(reason)) return "high";
  if (reason && NORMAL_DECISION.has(reason)) return "normal";
  return "medium";
}

function blockedAction(reason: BlockedReason | null): string {
  switch (reason) {
    case "permission_required":
      return "核对权限";
    case "needs_clarification":
      return "回答问题";
    case "auth_expired":
    case "review_failed":
    case "run_failed":
    case "agent_unresponsive":
      return "恢复任务";
    case "quality_gate":
    case "quality_regressed":
    case "review_block":
      return "查看门禁";
    case "budget_exceeded":
      return "调整预算";
    case "recovery_failed":
    case "worktree_missing":
      return "打开修复";
    default:
      return "查看原因";
  }
}

export function attentionItem(task: TaskSummary): AttentionItem | null {
  if (task.status === "WAITING_FOR_PLAN_APPROVAL") {
    return {
      task,
      kind: "approval",
      severity: "normal",
      reason: "计划已生成，等待批准",
      action: "审阅计划",
      groupKey: "plan-approval",
    };
  }
  if (task.status === "WAITING_FOR_HUMAN_APPROVAL") {
    return {
      task,
      kind: "approval",
      severity: "normal",
      reason: "开发、验证和审查已结束",
      action: "核对验收",
      groupKey: "final-approval",
    };
  }
  if (task.status === "APPROVED") {
    return {
      task,
      kind: "delivery",
      severity: "normal",
      reason: "验收已通过，尚未进入目标分支",
      action: "完成交付",
      groupKey: "delivery",
    };
  }
  if (task.status === "MERGE_CONFLICT") {
    return {
      task,
      kind: "conflict",
      severity: "high",
      reason: "目标分支存在合并冲突",
      action: "处理冲突",
      groupKey: "merge-conflict",
    };
  }
  if (task.status === "BLOCKED") {
    const reason = task.blockedReason;
    const blocked = reason ? BLOCKED_COPY[reason] : null;
    return {
      task,
      kind: reason === "permission_required" ? "permission" : "recovery",
      severity: blockedSeverity(reason),
      reason: blocked?.title ?? "任务已安全暂停",
      action: blockedAction(reason),
      groupKey: reason ?? "blocked",
    };
  }
  return null;
}

export function buildAttentionItems(tasks: TaskSummary[]): AttentionItem[] {
  return tasks
    .map(attentionItem)
    .filter((item): item is AttentionItem => item != null)
    .sort((a, b) => {
      const severity = SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity];
      if (severity !== 0) return severity;
      const age = Date.parse(a.task.updatedAt) - Date.parse(b.task.updatedAt);
      return age !== 0 ? age : a.task.seq - b.task.seq;
    });
}

export function groupAttentionItems(items: AttentionItem[]): AttentionGroup[] {
  const groups = new Map<string, AttentionGroup>();
  for (const item of items) {
    const group = groups.get(item.groupKey);
    if (group) group.items.push(item);
    else groups.set(item.groupKey, { key: item.groupKey, label: item.reason, items: [item] });
  }
  return [...groups.values()];
}
