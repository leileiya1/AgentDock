import { agentLabel } from "@/copy/agents";

/**
 * Backend event names must never reach the user (05 §10). This module is the only
 * place that turns an `event_type` string into 业务阶段 + 中文文案 + 状态。
 * Everything the user sees in the execution tree comes from here.
 */

/** 业务阶段 (05 §3.1). `null` means the event belongs in 系统详情, not the main tree. */
export type Phase = "plan" | "develop" | "validate" | "review" | "approval" | "delivery";

export const PHASE_ORDER: Phase[] = ["plan", "develop", "validate", "review", "approval", "delivery"];

export const PHASE_LABEL: Record<Phase, string> = {
  plan: "计划",
  develop: "开发",
  validate: "验证",
  review: "审查",
  approval: "人工批准",
  delivery: "交付",
};

/**
 * 执行结果通道 (05 §3.2). Kept separate from Agent 身份 and 业务阶段 so a node never
 * encodes two meanings in one dot. Every state carries an icon AND text at render time,
 * so 黑白截图 and 色弱模式 stay readable.
 */
export type NodeState = "pending" | "running" | "ok" | "attention" | "failed" | "info";

export interface EventCopy {
  /** `null` → 系统详情 (scheduler / heartbeat / result 文件 / 内部同步). */
  phase: Phase | null;
  label: string;
  state: NodeState;
  /**
   * `run` — 挂在原 run 下（Provider 降级、结构修复）；
   * `recovery` — 挂在原 run 的恢复分支下（daemon 重连接管）；
   * `phase` — 阶段级节点。
   */
  attach: "phase" | "run" | "recovery";
  /** 一句话解释「为什么」，用于降级、阻断等需要用户理解的节点。 */
  detail?: string;
}

type Payload = Record<string, unknown> | null | undefined;

function str(payload: Payload, key: string): string | null {
  const value = payload?.[key];
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function agentOf(payload: Payload, key: string): string | null {
  const raw = str(payload, key);
  return raw ? agentLabel(raw) : null;
}

/** Exact backend event types. Anything not listed falls through to `heuristicCopy`. */
const TABLE: Record<string, (p: Payload) => EventCopy> = {
  "user:start": () => ({ phase: null, label: "你启动了任务", state: "info", attach: "phase" }),
  "privacy:api_egress_approved": () => ({
    phase: null,
    label: "你允许了 API 数据外发",
    state: "info",
    attach: "phase",
  }),

  // ── 计划 ──────────────────────────────────────────────────────────
  "plan:proposed": () => ({ phase: "plan", label: "计划已生成，等待你批准", state: "attention", attach: "phase" }),
  "human:plan_approve": () => ({ phase: "plan", label: "你批准了计划，计划已锁定", state: "ok", attach: "phase" }),
  "human:plan_reject": (p) => ({
    phase: "plan",
    label: "你驳回了计划",
    state: "attention",
    attach: "phase",
    detail: str(p, "reason") ?? undefined,
  }),
  "plan:deviation": (p) => ({
    phase: "plan",
    label: "改动超出计划范围，需要重新批准",
    state: "attention",
    attach: "phase",
    detail: str(p, "detail") ?? str(p, "path") ?? undefined,
  }),

  // ── 开发 ──────────────────────────────────────────────────────────
  "run:succeeded": (p) => ({
    phase: "develop",
    label: "开发完成",
    state: "ok",
    attach: "phase",
    detail: str(p, "summary") ?? undefined,
  }),
  "provider:fallback": (p) => {
    const from = agentOf(p, "from");
    const to = agentOf(p, "to");
    return {
      phase: null, // 由 tree.ts 挂到原 run 下，而不是独立顶层事件 (05 §3.1)
      label: from && to ? `${from} 不可用，已降级到 ${to}` : "已切换备用 Provider",
      state: "attention",
      attach: "run",
      detail: str(p, "reason") ?? undefined,
    };
  },
  "provider:started": (p) => ({
    phase: null,
    label: `${agentOf(p, "agent") ?? "Provider"} 已开始${roleAction(str(p, "role"))}`,
    state: "running",
    attach: "phase",
    detail: "点击可查看对应执行日志",
  }),
  "provider:succeeded": (p) => ({
    phase: null,
    label: `${agentOf(p, "agent") ?? "Provider"}${roleAction(str(p, "role"))}完成`,
    state: "ok",
    attach: "phase",
    detail: "点击可查看对应执行日志",
  }),
  "provider:failed": (p) => ({
    phase: null,
    label: `${agentOf(p, "agent") ?? "Provider"}${roleAction(str(p, "role"))}失败`,
    state: "failed",
    attach: "phase",
    detail: str(p, "detail") ?? (typeof p?.exit_code === "number" ? `退出代码 ${p.exit_code}` : "点击查看对应执行日志"),
  }),
  "task:blocked": (p) => ({
    phase: "approval",
    label: "自动执行已阻断，等待你处理",
    state: "attention",
    attach: "phase",
    detail: str(p, "detail") ?? str(p, "reason") ?? undefined,
  }),

  // ── 审查 ──────────────────────────────────────────────────────────
  "review:pass": () => ({ phase: "review", label: "审查通过", state: "ok", attach: "phase" }),
  "review:request_changes": () => ({ phase: "review", label: "审查要求返工", state: "attention", attach: "phase" }),
  "review:block": () => ({ phase: "review", label: "审查拦截了本轮改动", state: "failed", attach: "phase" }),
  "review:failed": (p) => ({
    phase: "review",
    label: "审查未能完成",
    state: "failed",
    attach: "phase",
    detail: str(p, "error") ?? str(p, "detail") ?? undefined,
  }),
  "review:max_revisions": () => ({ phase: "review", label: "已达返工上限，需要你处理", state: "attention", attach: "phase" }),
  "review:council_pass": () => ({ phase: "review", label: "审查委员会通过", state: "ok", attach: "phase" }),
  "review:council_request_changes": () => ({
    phase: "review",
    label: "审查委员会要求返工",
    state: "attention",
    attach: "phase",
  }),
  "review:council_block": () => ({ phase: "review", label: "审查委员会拦截", state: "failed", attach: "phase" }),
  "review:council_member_failed": (p) => {
    const agent = agentOf(p, "agent");
    return {
      phase: "review",
      // Provider 故障不是反对票 (05 §6.11)，文案必须区分开。
      label: agent ? `${agent} 未能返回审查（Provider 故障，不计为反对票）` : "有成员未能返回审查",
      state: "failed",
      attach: "run",
      detail: str(p, "error") ?? undefined,
    };
  },
  "review:issues_resolved": () => ({ phase: "review", label: "上一轮问题已确认修复", state: "ok", attach: "phase" }),
  "integrity:security_review_required": (p) => ({
    phase: "review",
    label: "触发安全与完整性复审",
    state: "attention",
    attach: "phase",
    detail: str(p, "detail") ?? undefined,
  }),
  "quality:gate_failed": (p) => ({
    phase: "review",
    label: "质量门禁未通过",
    state: "failed",
    attach: "phase",
    detail: str(p, "detail") ?? str(p, "reason") ?? undefined,
  }),
  "quality:replayed": () => ({ phase: "review", label: "固定提交复验完成", state: "ok", attach: "phase" }),

  // ── 人工批准 ──────────────────────────────────────────────────────
  "human:approve": () => ({ phase: "approval", label: "你批准了本轮改动", state: "ok", attach: "phase" }),
  "human:force_approve": () => ({ phase: "approval", label: "你强制批准了本轮改动", state: "attention", attach: "phase" }),
  "human:reject": (p) => ({
    phase: "approval",
    label: "你驳回并要求返工",
    state: "attention",
    attach: "phase",
    detail: str(p, "reason") ?? undefined,
  }),
  "human:resume_with_guidance": (p) => ({
    phase: "approval",
    label: "你补充了指引",
    state: "info",
    attach: "phase",
    detail: str(p, "guidance") ?? undefined,
  }),
  "human:cancel": () => ({ phase: "approval", label: "你取消了任务", state: "info", attach: "phase" }),
  "human:budget_extended": () => ({ phase: "approval", label: "你提高了预算上限", state: "ok", attach: "phase" }),
  "budget:exceeded": (p) => ({
    phase: "approval",
    label: "预算已用完，任务已安全停在检查点",
    state: "attention",
    attach: "phase",
    detail: str(p, "detail") ?? str(p, "kind") ?? undefined,
  }),

  // ── 交付 ──────────────────────────────────────────────────────────
  "human:merge": () => ({ phase: "delivery", label: "开始合并", state: "running", attach: "phase" }),
  "merge:succeeded": () => ({ phase: "delivery", label: "已合并到目标分支", state: "ok", attach: "phase" }),
  "merge:conflict": (p) => ({
    phase: "delivery",
    label: "合并冲突，需要你处理",
    state: "attention",
    attach: "phase",
    detail: str(p, "detail") ?? undefined,
  }),
  "human:mark_merged_external": () => ({ phase: "delivery", label: "你标记为已在外部合并", state: "ok", attach: "phase" }),
  "delivery:change_request_opened": (p) => ({
    phase: "delivery",
    label: "已创建 PR / MR，等待 CI",
    state: "running",
    attach: "phase",
    detail: str(p, "url") ?? undefined,
  }),
  "delivery:ci_failed": (p) => ({
    phase: "delivery",
    label: "CI 未通过，因此没有合并",
    state: "failed",
    attach: "phase",
    detail: str(p, "detail") ?? str(p, "check") ?? undefined,
  }),
  "delivery:merged": () => ({ phase: "delivery", label: "远端变更已合并", state: "ok", attach: "phase" }),
  "delivery:head_mismatch": (p) => ({
    phase: "delivery",
    label: "远端分支已前进，需要重新检查",
    state: "attention",
    attach: "phase",
    detail: str(p, "detail") ?? undefined,
  }),
  "human:rollback": (p) => ({
    phase: "delivery",
    label: str(p, "strategy") === "revert" ? "你创建了回滚提交" : "你撤销了本地合并",
    state: "info",
    attach: "phase",
    detail: str(p, "commit") ?? undefined,
  }),

  // ── 恢复接管：挂在原 run 的恢复分支下 (05 §6.8) ────────────────────
  "recovery:run_adopted": () => ({
    phase: null,
    label: "正在确认这个 Agent 是否仍在运行",
    state: "running",
    attach: "recovery",
  }),
  "recovery:run_recovered": () => ({
    phase: null,
    label: "已重新接管，日志继续同步",
    state: "ok",
    attach: "recovery",
  }),
  "recovery:run_adoption_failed": (p) => ({
    phase: null,
    label: "未能接管这次运行",
    state: "failed",
    attach: "recovery",
    detail: adoptionFailureReason(str(p, "detail")),
  }),
  "recovery:interrupted": () => ({
    phase: null,
    label: "后台服务重启，本次运行已重新排队",
    state: "attention",
    attach: "recovery",
  }),
};

function roleAction(role: string | null): string {
  if (role === "planner") return "规划";
  if (role === "reviewer") return "审查";
  if (role === "validator") return "验证";
  return "开发";
}

/**
 * 接管失败必须区分原因 (05 §6.8)：进程已退出 / 身份不匹配 / 日志不可读 / 结果待校验。
 * 后端 detail 是英文技术串，这里翻译成用户语言。
 */
function adoptionFailureReason(detail: string | null): string | undefined {
  if (!detail) return undefined;
  const lower = detail.toLowerCase();
  if (lower.includes("identity mismatch")) return "结果与本任务不匹配，已丢弃，需要重跑这一步";
  if (lower.includes("role") || lower.includes("schema")) return "结果格式无法校验，需要重跑这一步";
  if (lower.includes("exit")) return "进程已经退出，没有留下可用结果";
  if (lower.includes("log") || lower.includes("read")) return "运行日志不可读，无法确认进度";
  return detail;
}

/** 默认进入系统详情的内部事件前缀 (05 §3.3)。 */
const SYSTEM_PREFIXES = ["scheduler:", "result:", "storage:", "project_config:", "onboarding:", "recovery:"];

/**
 * Legacy rows and Providers we have not enumerated still need a phase, but they must
 * never render a raw `event_type`. Keyword matching stays conservative: anything we
 * cannot confidently name goes to 系统详情 rather than guessing user-facing copy.
 */
function heuristicCopy(eventType: string, payload: Payload): EventCopy {
  const t = eventType.toLowerCase();
  const has = (s: string) => t.includes(s);
  const failed = has("fail") || has("error") || has("timeout") || has("interrupt");
  const done = has("succeed") || has("success") || has("complete") || has("pass") || has("done");
  const started = has("start") || has("begin");
  const state: NodeState = failed ? "failed" : done ? "ok" : started ? "running" : "info";

  if (SYSTEM_PREFIXES.some((prefix) => t.startsWith(prefix))) {
    return { phase: null, label: systemLabel(eventType), state, attach: "phase" };
  }
  if (has("plan")) return { phase: "plan", label: `计划${verb(state)}`, state, attach: "phase" };
  if (has("valid") || (has("test") && !has("request"))) {
    return { phase: "validate", label: `验证${verb(state)}`, state, attach: "phase" };
  }
  if (has("review")) return { phase: "review", label: `审查${verb(state)}`, state, attach: "phase" };
  if (has("merg") || has("deliver") || has("rollback")) {
    return { phase: "delivery", label: `交付${verb(state)}`, state, attach: "phase" };
  }
  if (has("approv") || has("reject") || has("wait") || has("block") || has("clarif")) {
    return { phase: "approval", label: has("block") || has("clarif") ? "需要你处理" : "等待你确认", state: "attention", attach: "phase" };
  }
  if (has("develop") || has("revis") || has("run")) {
    const base = has("revis") ? "返工" : "开发";
    return {
      phase: "develop",
      label: `${base}${verb(state)}`,
      state,
      attach: "phase",
      detail: str(payload, "summary") ?? undefined,
    };
  }
  if (has("creat")) return { phase: null, label: "任务已创建", state: "info", attach: "phase" };
  return { phase: null, label: systemLabel(eventType), state, attach: "phase" };
}

function verb(state: NodeState): string {
  if (state === "ok") return "完成";
  if (state === "failed") return "失败";
  if (state === "running") return "进行中";
  return "";
}

/** 系统详情里也不显示原始事件名，给一个中性可读标签。 */
function systemLabel(eventType: string): string {
  const [group, rest = ""] = eventType.split(":");
  const groupLabel: Record<string, string> = {
    scheduler: "调度",
    result: "结果文件",
    recovery: "后台恢复",
    storage: "本地存储",
    project_config: "项目配置信任",
    onboarding: "初始化",
    privacy: "数据外发",
  };
  const action = rest.replace(/_/g, " ").trim();
  return `${groupLabel[group] ?? "系统"}${action ? ` · ${action}` : ""}`;
}

export function eventCopy(eventType: string, payload: Payload): EventCopy {
  const exact = TABLE[eventType];
  if (exact) return exact(payload);
  // `result:repair_*` is built with format!(), so match by prefix rather than listing every outcome.
  if (eventType.startsWith("result:repair")) {
    const outcome = eventType.endsWith("succeeded") ? "ok" : eventType.endsWith("failed") ? "failed" : "running";
    return {
      phase: null,
      label:
        outcome === "ok" ? "结构化结果已修复" : outcome === "failed" ? "结构化结果修复失败" : "正在修复结构化结果",
      state: outcome as NodeState,
      attach: "run",
      detail: str(payload, "detail") ?? undefined,
    };
  }
  return heuristicCopy(eventType, payload);
}

/**
 * `scheduler:slot` 等内部事件是否默认隐藏 (05 §3.3)。挂在 run / 恢复分支下的事件
 * （Provider 降级、接管结果）虽然没有独立阶段，但对用户有意义，不算系统详情。
 */
export function isSystemDetail(copy: EventCopy): boolean {
  return copy.phase === null && copy.attach === "phase";
}
