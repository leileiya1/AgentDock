import type {
  PermissionActionType,
  PermissionOperation,
  PermissionRule,
  RunRole,
  AgentKind,
} from "@/generated/bindings";
import { actionCopy } from "@/copy/permission";

/**
 * 项目规则派生 (06 §9 第 13/17/18/19 条). 「保存项目规则」必须二次展示后端实际会保存的
 * 精确能力，而不是一句「以后允许」。这里从请求的规范化 operation 推导可解释的规则预览，
 * 并为规则列表提供状态/命中/到期的展示逻辑。
 */

export interface RuleLine {
  label: string;
  /** 等宽呈现的具体值（argv、glob、域名等）。 */
  value: string;
  mono?: boolean;
}

/** 生成规则预览所需的最小字段——请求与既有规则都满足这个形状。 */
export interface RulePreviewInput {
  providerId: AgentKind;
  role: RunRole;
  actionType: PermissionActionType;
  operation: PermissionOperation;
}

/**
 * 从一条权限请求（或既有规则）推导「保存项目规则」会写入的精确规则预览 (§9 第 13 条)。
 * 只呈现精确能力：可执行文件与参数模板、允许的 cwd 与相对路径、精确域名端口、环境变量名。
 * 绝不生成 `*` 命令、`/**` 路径或任意域名——那些形式后端本就禁止持久化。
 */
export function rulePreview(request: RulePreviewInput): RuleLine[] {
  const op = request.operation;
  const lines: RuleLine[] = [];
  const meta = actionCopy(request.actionType);

  lines.push({ label: "Provider / 角色", value: `${request.providerId} · ${request.role}` });
  lines.push({ label: "动作类型", value: meta.label });

  if (op.argv.length > 0) {
    lines.push({ label: "可执行文件", value: op.argv[0], mono: true });
    if (op.argv.length > 1) {
      lines.push({ label: "参数模板", value: op.argv.slice(1).join(" "), mono: true });
    }
  }
  if (op.cwd) lines.push({ label: "执行目录", value: op.cwd, mono: true });
  for (const path of op.paths) {
    lines.push({
      label: path.outsideWorktree ? "工作树外路径" : "允许路径",
      value: `${path.path} (${accessLabel(path.access)})`,
      mono: true,
    });
  }
  for (const domain of op.networkDomains) {
    lines.push({ label: "允许域名", value: domain, mono: true });
  }
  for (const name of op.environmentNames) {
    lines.push({ label: "允许环境变量", value: name, mono: true });
  }
  return lines;
}

function accessLabel(access: "read" | "write" | "delete"): string {
  return access === "read" ? "只读" : access === "write" ? "写入" : "删除";
}

export type RuleStatus = "active" | "revoked" | "disabled" | "expired";

/** 规则当前状态 (§9 第 18 条)：撤销 / 停用 / 到期 / 生效。撤销与停用优先于到期展示。 */
export function ruleStatus(rule: PermissionRule, now: number = Date.now()): RuleStatus {
  if (rule.revokedAt) return "revoked";
  if (!rule.enabled) return "disabled";
  if (rule.expiresAt && Date.parse(rule.expiresAt) <= now) return "expired";
  return "active";
}

export const RULE_STATUS_COPY: Record<RuleStatus, { label: string; tone: "ok" | "idle" | "bad" }> = {
  active: { label: "生效中", tone: "ok" },
  revoked: { label: "已撤销", tone: "bad" },
  disabled: { label: "已停用", tone: "idle" },
  expired: { label: "已过期", tone: "idle" },
};

/** 规则能否被下一次执行命中——只有 active 才会放行 (§9 第 19 条)。 */
export function ruleIsEffective(rule: PermissionRule, now: number = Date.now()): boolean {
  return ruleStatus(rule, now) === "active";
}

/**
 * 规则列表展示排序：生效中优先，其次按最近命中时间倒序，再按创建时间倒序。
 * 撤销/停用/过期的规则沉底，便于用户先看到当前真正生效的规则。
 */
export function sortRules(rules: PermissionRule[], now: number = Date.now()): PermissionRule[] {
  const rank = (rule: PermissionRule) => (ruleStatus(rule, now) === "active" ? 0 : 1);
  return rules.slice().sort((a, b) => {
    const byActive = rank(a) - rank(b);
    if (byActive !== 0) return byActive;
    const aHit = a.lastMatchedAt ? Date.parse(a.lastMatchedAt) : 0;
    const bHit = b.lastMatchedAt ? Date.parse(b.lastMatchedAt) : 0;
    if (aHit !== bHit) return bHit - aHit;
    return Date.parse(b.createdAt) - Date.parse(a.createdAt);
  });
}

export interface RuleSummary {
  total: number;
  active: number;
}

export function summarizeRules(rules: PermissionRule[] | undefined, now: number = Date.now()): RuleSummary {
  const list = rules ?? [];
  return { total: list.length, active: list.filter((r) => ruleIsEffective(r, now)).length };
}
