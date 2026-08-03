import type {
  PermissionActionType,
  PermissionDecisionKind,
  PermissionGrantScope,
  PermissionRequestStatus,
  PermissionRiskLevel,
  RunRole,
} from "@/generated/bindings";

/** 角色文案，用于「谁在请求」展示。 */
export const ROLE_LABEL: Record<RunRole, string> = {
  planner: "规划",
  developer: "开发",
  reviewer: "审查",
  validator: "验证",
};

export function roleLabel(role: RunRole): string {
  return ROLE_LABEL[role] ?? role;
}

/**
 * 权限代理桌面文案 (06 §9). 前端绝不根据自然语言猜测风险或可授权性——所有判断都来自
 * 后端稳定的枚举 (action_type / risk_level / grantable)。这里只把这些枚举翻译成用户
 * 能判断「谁、想做什么、在哪、会访问什么、为什么」的中文，并明确「模型 API 外发」与
 * 「Agent 命令访问网络」的区别 (§9 第 9 条)。
 */

export interface ActionCopy {
  /** 短标题，等宽命令之外的人话说明 (§9 第 6 条)。 */
  label: string;
  /** 一句话解释这个动作类别在做什么。 */
  hint: string;
  /** 命令类动作用等宽 argv 呈现，其它类别不需要。 */
  showsArgv: boolean;
  /** 网络类动作需要区分模型外发 / Agent 命令网络 (§9 第 9 条)。 */
  isNetwork: boolean;
}

/** 稳定 action_type → 文案。key 与后端 PermissionActionType 完全对应。 */
export const ACTION_COPY: Record<PermissionActionType, ActionCopy> = {
  worktree_read: { label: "读取任务文件", hint: "读取当前任务工作树内的文件。", showsArgv: false, isNetwork: false },
  worktree_write: { label: "修改任务文件", hint: "修改当前任务工作树内的源码或测试。", showsArgv: false, isNetwork: false },
  worktree_delete: { label: "删除任务文件", hint: "删除当前任务工作树内的文件。", showsArgv: false, isNetwork: false },
  control_plane_write: { label: "修改控制面配置", hint: "改动 .agentflow、CI、hooks 或权限配置——等于改规则本身。", showsArgv: false, isNetwork: false },
  command_execute: { label: "执行命令", hint: "运行一条命令（测试、格式化、构建等）。", showsArgv: true, isNetwork: false },
  dependency_install: { label: "安装依赖", hint: "从依赖仓库下载并安装依赖包。", showsArgv: true, isNetwork: true },
  network_access: { label: "访问网络", hint: "由 Agent 命令发起的对外网络请求。", showsArgv: false, isNetwork: true },
  environment_read: { label: "读取环境变量", hint: "读取被允许的环境变量名称。", showsArgv: false, isNetwork: false },
  secret_access: { label: "访问密钥", hint: "读取 token、Keychain 或 SSH key 等敏感凭据。", showsArgv: false, isNetwork: false },
  process_control: { label: "控制子进程", hint: "启动或终止子进程。", showsArgv: true, isNetwork: false },
  git_read: { label: "读取 Git 信息", hint: "执行 status、diff、log、show 等只读 Git 操作。", showsArgv: true, isNetwork: false },
  git_mutation: { label: "改写 Git 状态", hint: "commit、reset、clean、push、tag 等改写操作。", showsArgv: true, isNetwork: false },
  system_change: { label: "修改系统", hint: "sudo、系统设置或启动项——属于不可授权项。", showsArgv: true, isNetwork: false },
  external_path: { label: "访问工作树外路径", hint: "读取或写入当前任务工作树之外的路径。", showsArgv: false, isNetwork: false },
};

export function actionCopy(action: PermissionActionType): ActionCopy {
  return (
    ACTION_COPY[action] ?? {
      label: action,
      hint: "未知动作类型，按最保守方式处理。",
      showsArgv: false,
      isNetwork: false,
    }
  );
}

export type RiskTone = "ok" | "caution" | "human" | "bad";

export interface RiskCopy {
  label: string;
  /** 语义色调，用于文字与图标（绝不只用颜色，§9 第 22 条）。 */
  tone: RiskTone;
  /** 一句话说明为什么是这个风险级别。 */
  why: string;
}

export const RISK_COPY: Record<PermissionRiskLevel, RiskCopy> = {
  low: { label: "低风险", tone: "ok", why: "影响范围限于当前任务，可放心处理。" },
  medium: { label: "中风险", tone: "caution", why: "会触及依赖、网络或配置，请核对具体内容。" },
  high: { label: "高风险", tone: "human", why: "可能影响工作树外、密钥或不可逆操作，仅提供一次/本任务授权。" },
  forbidden: { label: "不可授权", tone: "bad", why: "属于系统级越界能力，AgentFlow 不会为其提供允许按钮。" },
};

export function riskCopy(risk: PermissionRiskLevel): RiskCopy {
  return RISK_COPY[risk] ?? RISK_COPY.high;
}

/** 授权范围文案 (§9 第 10/13 条)。 */
export interface ScopeCopy {
  label: string;
  hint: string;
}

export const SCOPE_COPY: Record<PermissionGrantScope, ScopeCopy> = {
  once: { label: "允许一次", hint: "仅放行这一次执行，下一次相同请求仍会再次询问。" },
  task: { label: "本任务允许", hint: "在当前任务有效期内自动放行，其它任务不复用。" },
  project_rule: { label: "保存项目规则", hint: "为当前项目保存一条精确规则，之后匹配同一命令/路径/域名时自动放行。" },
};

/** 决定按钮文案 (§9 第 10 条)。approve 的具体标题由 scope 决定，这里给 deny/cancel。 */
export const DECISION_COPY: Record<PermissionDecisionKind, string> = {
  approve: "允许",
  deny: "拒绝并补充指引",
  cancel_task: "取消任务",
};

/** 请求状态文案（用于列表与审计，§9 第 20/21 条）。 */
export interface StatusCopy {
  label: string;
  tone: RiskTone | "idle";
}

export const REQUEST_STATUS_COPY: Record<PermissionRequestStatus, StatusCopy> = {
  pending: { label: "等待你授权", tone: "human" },
  approved: { label: "已允许", tone: "ok" },
  denied: { label: "已拒绝", tone: "bad" },
  cancelled: { label: "已取消", tone: "idle" },
  expired: { label: "已过期", tone: "idle" },
};

export function requestStatusCopy(status: PermissionRequestStatus): StatusCopy {
  return REQUEST_STATUS_COPY[status] ?? { label: status, tone: "idle" };
}

/**
 * 网络用途区分 (§9 第 9 条)：dependency_install / network_access 都是「Agent 命令访问网络」，
 * 与「模型 API 外发」是两条独立的审计与授权路径。权限请求只会承载 Agent 命令网络——模型
 * 外发不经过这个弹窗，因此这里显式标注，避免用户把两者混为一谈。
 */
export const NETWORK_KIND = {
  agentCommand: {
    label: "Agent 命令访问网络",
    hint: "这是 Agent 运行的命令要连接的外部地址，与模型推理的数据外发是两回事。",
  },
  modelEgress: {
    label: "模型 API 外发",
    hint: "模型推理产生的数据外发，由独立的隐私授权与审计通道处理，不在此弹窗内。",
  },
} as const;
