import type { BlockedReason } from "@/generated/bindings";

/** Actions a BLOCKED task can offer, mapped to real commands (02 §4.4). */
export type BlockedAction =
  /** free-form guidance → taskResumeWithGuidance */
  | "guidance"
  /** answer the agent's question → taskResumeWithGuidance (clarification framing) */
  | "answer"
  /** push straight to approval → taskForceApprove */
  | "forceApprove"
  /** give up → taskCancel */
  | "cancel"
  /** inspect checkpoints and run a bounded repair action */
  | "repair"
  /** increase or remove task budgets and resume from the saved scheduler checkpoint */
  | "budget";

export interface BlockedCopy {
  title: string;
  /** 人话解释 — what happened, in the user's terms. */
  explanation: string;
  /** whether blockedDetail holds a question to surface prominently */
  detailIsQuestion: boolean;
  actions: BlockedAction[];
}

/** BlockedReason → 人话 + 合理动作组合 (02 §4.4). */
export const BLOCKED_COPY: Record<BlockedReason, BlockedCopy> = {
  needs_clarification: {
    title: "开发 Agent 需要你澄清",
    explanation: "它在动手前有一个问题，回答后会带着你的说明继续。",
    detailIsQuestion: true,
    actions: ["answer", "cancel"],
  },
  max_revisions: {
    title: "返工次数到上限了",
    explanation:
      "连续几轮审查仍要求返工，已达到你设置的最大轮数。你可以补充指引再试、直接送去人工批准，或取消。",
    detailIsQuestion: false,
    actions: ["guidance", "forceApprove", "cancel"],
  },
  review_block: {
    title: "审查判定必须拦截",
    explanation:
      "审查 Agent 认为存在不能放行的问题。补充指引后可以让开发重来，或直接取消。",
    detailIsQuestion: false,
    actions: ["guidance", "cancel"],
  },
  review_failed: {
    title: "审查没能完成",
    explanation:
      "审查运行本身失败了（例如额度用尽或输出不合法）。恢复后可以补充指引重试，或取消任务。",
    detailIsQuestion: false,
    actions: ["guidance", "cancel"],
  },
  run_failed: {
    title: "这一轮运行失败了",
    explanation:
      "开发或验证运行没有正常结束。补充一点指引后可以再试一轮，或取消任务。",
    detailIsQuestion: false,
    actions: ["guidance", "cancel"],
  },
  no_changes: {
    title: "这一轮没有产生任何改动",
    explanation:
      "开发 Agent 完成后工作树没有变化。可能是需求已满足，或说明不够具体——补充指引后重试，或取消。",
    detailIsQuestion: false,
    actions: ["guidance", "cancel"],
  },
  validation_infra: {
    title: "验证环境出了问题",
    explanation:
      "验证步骤没能正常运行（不是代码本身失败，而是运行验证的环境）。检查 .agentflow/project.toml 里的验证命令后重试。",
    detailIsQuestion: false,
    actions: ["guidance", "cancel"],
  },
  worktree_missing: {
    title: "任务的工作树丢失了",
    explanation:
      "找不到这个任务的隔离工作目录（可能被外部清理了）。补充指引重试会尝试继续，否则可以取消任务。",
    detailIsQuestion: false,
    actions: ["repair", "cancel"],
  },
  commit_guard: {
    title: "提交保护拦截了这一轮",
    explanation:
      "检测到依赖目录、凭据文件、超大文件或异常多的改动，因此没有创建提交。改动仍保留在隔离工作树中，清理危险文件并补充指引后即可继续。",
    detailIsQuestion: false,
    actions: ["guidance", "cancel"],
  },
  budget_exceeded: {
    title: "任务预算已用完",
    explanation: "AgentFlow 已停止继续调度，避免产生超出预期的 Token、费用或运行时间。可在治理页核对用量后再决定是否继续。",
    detailIsQuestion: false,
    actions: ["budget", "cancel"],
  },
  remote_node_unavailable: {
    title: "远程执行节点不可用",
    explanation: "SSH 健康检查、远端工作目录或 Git 环境未通过。修复节点连接后可补充指引重试，代码提交仍保留。",
    detailIsQuestion: false,
    actions: ["guidance", "cancel"],
  },
  ci_failed: {
    title: "远端 CI 没有通过",
    explanation: "PR / MR 已保留，但当前检查不满足合并门禁。查看远端日志并补充指引后再继续。",
    detailIsQuestion: false,
    actions: ["guidance", "cancel"],
  },
  quality_gate: {
    title: "质量门禁没有通过",
    explanation: "自动验证、独立审查、高风险问题或控制面变更使质量分低于任务阈值。查看治理页后返工，必要时可明确人工放行。",
    detailIsQuestion: false,
    actions: ["guidance", "forceApprove", "cancel"],
  },
};

export const BLOCKED_ACTION_LABEL: Record<BlockedAction, string> = {
  guidance: "补充指引继续",
  answer: "回答并继续",
  forceApprove: "直接送去批准",
  cancel: "取消任务",
  repair: "打开修复中心",
  budget: "调整预算继续",
};
