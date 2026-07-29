import type { ReviewDecision, TaskDetail } from "@/generated/bindings";
import { BLOCKED_COPY } from "@/copy/blocked";
import { STATUS_COPY } from "@/copy/status";
import { acceptanceStatus, type AcceptanceStatus } from "@/lib/acceptance";
import type { ValidationOutcome } from "@/lib/execution/testReport";

export interface AcceptanceCounts {
  passed: number;
  failed: number;
  pending: number;
  unverified: number;
  manual: number;
  total: number;
}

export function acceptanceCounts(
  task: TaskDetail,
  validation: ValidationOutcome,
  review: ReviewDecision | null
): AcceptanceCounts {
  const counts: Record<AcceptanceStatus, number> = {
    passed: 0,
    failed: 0,
    pending: 0,
    unverified: 0,
    manual: 0,
  };
  for (const criterion of task.acceptanceCriteria) {
    counts[acceptanceStatus(criterion.kind, validation, review)] += 1;
  }
  return { ...counts, total: task.acceptanceCriteria.length };
}

export function resultHeadline(task: TaskDetail): { title: string; detail: string; tone: "ok" | "human" | "run" | "idle" } {
  if (task.status === "MERGED") {
    return { title: "改动已交付", detail: `已合并到 ${task.targetBranch}，本任务流程结束。`, tone: "ok" };
  }
  if (task.status === "WAITING_FOR_HUMAN_APPROVAL") {
    return { title: "自动步骤已结束，等你验收", detail: "先核对下面的条件、验证和审查证据，再决定批准或返工。", tone: "human" };
  }
  if (task.status === "APPROVED") {
    return { title: "验收已通过，等待交付", detail: `改动尚未进入 ${task.targetBranch}。`, tone: "human" };
  }
  if (task.status === "BLOCKED" && task.blockedReason) {
    const copy = BLOCKED_COPY[task.blockedReason];
    return { title: copy.title, detail: copy.explanation, tone: "human" };
  }
  if (task.status === "MERGE_CONFLICT") {
    return { title: "交付时遇到合并冲突", detail: "改动仍然保留，需要处理冲突后再交付。", tone: "human" };
  }
  if (["DEVELOPING", "REVISING", "PLANNING", "VALIDATING", "REVIEWING", "MERGING"].includes(task.status)) {
    return { title: STATUS_COPY[task.status].label, detail: "结果尚未定稿，下面的证据会随着执行进展更新。", tone: "run" };
  }
  return { title: STATUS_COPY[task.status].label, detail: "这里汇总当前已有的结果与验收证据。", tone: "idle" };
}
