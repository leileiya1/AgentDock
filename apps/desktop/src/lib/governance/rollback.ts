import type {
  RollbackBlocker,
  RollbackPreflight,
  RollbackRecommendationReason,
  RollbackStrategy,
} from "@/generated/bindings";

export const ROLLBACK_BLOCKER_LABEL: Record<RollbackBlocker, string> = {
  task_not_merged: "任务尚未合并或已经回滚",
  delivery_record_missing: "缺少交付记录，无法确认回滚边界",
  remote_delivery_unsupported: "远端 PR/MR 暂不支持从本地安全回滚",
  target_branch_not_checked_out: "当前检出的不是任务目标分支",
  dirty_working_tree: "目标分支工作区有未提交改动",
  merge_commit_missing: "交付记录缺少 merge commit",
  pre_merge_commit_missing: "缺少合并前提交，不能安全撤销历史",
  later_commits_exist: "merge commit 之后已有新提交",
  merge_not_in_head: "当前分支不包含本任务的 merge commit",
};

const RECOMMENDATION_LABEL: Record<RollbackRecommendationReason, string> = {
  undo_exact_head: "目标分支仍停在本次合并，推荐直接撤销；不会产生额外提交。",
  revert_preserves_later_commits: "检测到后续提交，推荐创建反向提交以保留后来工作。",
  no_safe_strategy: "当前没有可安全执行的回滚方式，请先处理阻塞项。",
};

export function rollbackRecommendation(preflight: RollbackPreflight) {
  return RECOMMENDATION_LABEL[preflight.recommendationReason];
}

export function rollbackAllowed(preflight: RollbackPreflight, strategy: RollbackStrategy) {
  return strategy === "undo" ? preflight.canUndo : preflight.canRevert;
}

export function rollbackBlockers(preflight: RollbackPreflight, strategy: RollbackStrategy) {
  return (strategy === "undo" ? preflight.undoBlockers : preflight.revertBlockers).map(
    (blocker) => ROLLBACK_BLOCKER_LABEL[blocker],
  );
}

export function shortCommit(sha: string | null) {
  return sha ? sha.slice(0, 8) : "未知";
}
