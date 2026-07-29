import type { TaskSummary } from "@/generated/bindings";
import { BLOCKED_COPY } from "@/copy/blocked";

export type AttentionKind = "permission" | "approval" | "recovery" | "conflict" | "delivery";

export interface AttentionItem {
  task: TaskSummary;
  kind: AttentionKind;
  reason: string;
  nextStep: string;
}

export const ATTENTION_META: Record<AttentionKind, { label: string; action: string }> = {
  permission: { label: "权限决定", action: "核对权限" },
  approval: { label: "等待验收", action: "查看并决定" },
  recovery: { label: "运行受阻", action: "查看恢复方式" },
  conflict: { label: "合并冲突", action: "处理冲突" },
  delivery: { label: "等待交付", action: "完成交付" },
};

export function attentionItem(task: TaskSummary): AttentionItem | null {
  if (task.status === "WAITING_FOR_PLAN_APPROVAL") {
    return {
      task,
      kind: "approval",
      reason: "执行计划已经准备好，尚未开始修改代码。",
      nextStep: "检查范围和步骤，批准后再开始开发。",
    };
  }
  if (task.status === "WAITING_FOR_HUMAN_APPROVAL") {
    return {
      task,
      kind: "approval",
      reason: "自动开发、验证和审查已经结束，正在等待最终决定。",
      nextStep: "核对验收条件与证据，再批准、返工或取消。",
    };
  }
  if (task.status === "APPROVED") {
    return {
      task,
      kind: "delivery",
      reason: "任务已经通过验收，但改动还没有进入目标分支。",
      nextStep: "确认交付方式并完成合并。",
    };
  }
  if (task.status === "MERGE_CONFLICT") {
    return {
      task,
      kind: "conflict",
      reason: "目标分支发生变化，当前改动无法安全自动合并。",
      nextStep: "检查冲突文件，处理后重新执行交付。",
    };
  }
  if (task.status === "BLOCKED") {
    if (task.blockedReason === "permission_required") {
      return {
        task,
        kind: "permission",
        reason: BLOCKED_COPY.permission_required.explanation,
        nextStep: "核对将要执行的操作，明确允许或拒绝。",
      };
    }
    const blocked = task.blockedReason ? BLOCKED_COPY[task.blockedReason] : null;
    return {
      task,
      kind: "recovery",
      reason: blocked?.explanation ?? "任务已经安全暂停，需要你选择恢复方式。",
      nextStep: blocked?.detailIsQuestion
        ? "回答 Agent 的问题后继续。"
        : "查看失败原因，并选择重试、修复或取消。",
    };
  }
  return null;
}

export function buildAttentionItems(tasks: TaskSummary[]): AttentionItem[] {
  return tasks.map(attentionItem).filter((item): item is AttentionItem => item != null);
}
