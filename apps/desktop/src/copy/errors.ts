import type { AppError, ErrorCode } from "@/generated/bindings";

/**
 * ErrorCode → 人话文案 (03 §5). Principle (02 §8): 发生了什么 + 怎么办,
 * 不道歉不卖萌. `detail` from the backend may be appended for context.
 */
const ERROR_COPY: Record<ErrorCode, { title: string; hint?: string }> = {
  ENV_CLI_NOT_FOUND: {
    title: "没找到这个命令行工具",
    hint: "去设置里填写它的可执行文件路径，或安装后重新检测。",
  },
  ENV_CLI_INCOMPATIBLE: {
    title: "这个工具的版本不兼容",
    hint: "升级到受支持的版本后重新检测。",
  },
  CLI_INSTALL_FAILED: {
    title: "CLI 安装失败",
    hint: "检查网络与 npm 权限，或按官方文档手动安装。",
  },
  API_CREDENTIAL_FAILED: {
    title: "API 配置失败",
    hint: "检查钥匙串权限后重试。",
  },
  API_EGRESS_APPROVAL_REQUIRED: {
    title: "需要确认 API 数据外发",
    hint: "勾选外发授权后再创建；不授权时请选择 CLI 审查并仅使用 CLI 降级链。",
  },
  PROJECT_NOT_GIT: {
    title: "这个目录不是 Git 仓库",
    hint: "选择一个已经 git init 的项目根目录再导入。",
  },
  PROJECT_ALREADY_IMPORTED: {
    title: "这个项目已经导入过了",
    hint: "在左侧项目栏里就能找到它。",
  },
  TASK_INVALID_STATE: {
    title: "当前状态下不能做这个操作",
    hint: "任务状态已经变了，刷新后再试。",
  },
  TASK_SAME_AGENT: {
    title: "开发和审查不能用同一个 Agent",
    hint: "同源审查会显著降低缺陷检出——换一个审查 Agent。",
  },
  RUN_SPAWN_FAILED: {
    title: "没能启动这个运行",
    hint: "检查对应 CLI 的路径与登录状态。",
  },
  RESULT_INVALID_SCHEMA: {
    title: "Agent 的输出不符合约定格式",
    hint: "已自动重试一次；仍失败时可补充指引后继续。",
  },
  DIFF_STALE: {
    title: "内容已变化",
    hint: "已为你刷新到最新，请重新确认后再批准。",
  },
  MERGE_PRECONDITION_FAILED: {
    title: "现在还不能合并",
  },
  MERGE_CONFLICT: {
    title: "合并遇到冲突",
    hint: "可以重试合并，或手动解决后标记完成。",
  },
  WORKTREE_MISSING: {
    title: "工作树丢失了",
    hint: "重建工作树后即可继续。",
  },
  DB_ERROR: {
    title: "本地数据出了点问题",
    hint: "稍后重试；若持续出现，去设置里查看存储状态。",
  },
  PLAN_APPROVAL_REQUIRED: {
    title: "需要先批准编码计划",
    hint: "在计划确认框中批准，或驳回并说明要调整的地方。",
  },
  BUDGET_EXCEEDED: {
    title: "新预算仍不足",
    hint: "把上限调到高于当前已用量，或留空表示不限。",
  },
  QUALITY_GATE_FAILED: {
    title: "质量门禁没有通过",
    hint: "先查看治理页中的失败项，修复或复验后再交付。",
  },
  SCM_CLI_NOT_FOUND: {
    title: "缺少代码托管命令行工具",
    hint: "GitHub 模式安装并登录 gh；GitLab 模式安装并登录 glab。",
  },
  CI_FAILED: {
    title: "远端 CI 没有通过",
    hint: "打开 PR / MR 检查失败任务，修复后再刷新。",
  },
  REMOTE_NODE_UNAVAILABLE: {
    title: "远程执行节点不可用",
    hint: "在设置中检查 SSH 连接、工作目录权限和 Git 环境。",
  },
  ROLLBACK_UNSAFE: {
    title: "为保护现有改动，已拒绝回滚",
    hint: "确认目标分支工作区干净；存在后续提交时请使用回滚提交。",
  },
  IO_ERROR: {
    title: "读写文件失败",
    hint: "检查磁盘空间与目录权限后重试。",
  },
  INTERNAL: {
    title: "出现了内部错误",
    hint: "稍后重试。",
  },
};

export interface DisplayError {
  code: ErrorCode | "UNKNOWN";
  title: string;
  hint?: string;
  detail?: string | null;
  raw: unknown;
}

function isAppError(e: unknown): e is AppError {
  return (
    typeof e === "object" &&
    e !== null &&
    "code" in e &&
    "message" in e
  );
}

/** Normalize any command/thrown error into a display-ready shape. */
export function toAppError(e: unknown): DisplayError {
  if (isAppError(e)) {
    const copy = ERROR_COPY[e.code];
    if (copy) {
      return {
        code: e.code,
        title: copy.title,
        hint: copy.hint,
        detail: e.detail ?? null,
        raw: e,
      };
    }
    return { code: e.code, title: e.message || "操作失败", detail: e.detail ?? null, raw: e };
  }
  if (e instanceof Error) {
    return { code: "UNKNOWN", title: e.message || "操作失败", raw: e };
  }
  return { code: "UNKNOWN", title: "操作失败", raw: e };
}

/** One-line message suitable for a toast. */
export function errorLine(e: unknown): string {
  const d = toAppError(e);
  // MERGE_PRECONDITION_FAILED and friends carry a human reason in detail.
  if (d.detail) return `${d.title} · ${d.detail}`;
  if (d.hint) return `${d.title} · ${d.hint}`;
  return d.title;
}
