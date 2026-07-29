import type {
  AgentKind,
  PermissionActionType,
  PermissionRequest,
} from "@/generated/bindings";
import { actionCopy } from "@/copy/permission";

/**
 * 权限审计筛选与摘要 (06 §9 第 20/21 条). 纯函数：把任务的权限请求列表按 Provider、
 * 动作类型、决定、规则筛选，并给每条生成「请求/允许/拒绝」的一句话短摘要；技术 JSON
 * 由组件放进高级详情，不进入默认视图。
 */

/** 决定维度：与后端 status 对齐，但用审计语义分组（pending=待处理，approved=允许，denied=拒绝）。 */
export type AuditDecision = "all" | "pending" | "approved" | "denied" | "cancelled" | "expired";

export interface AuditFilter {
  provider: AgentKind | "all";
  actionType: PermissionActionType | "all";
  decision: AuditDecision;
  /** rule = 命中了项目规则；none = 未命中；all = 不限。 */
  rule: "all" | "matched" | "none";
}

export const EMPTY_FILTER: AuditFilter = {
  provider: "all",
  actionType: "all",
  decision: "all",
  rule: "all",
};

export function filterRequests(requests: PermissionRequest[], filter: AuditFilter): PermissionRequest[] {
  return requests.filter((r) => {
    if (filter.provider !== "all" && r.providerId !== filter.provider) return false;
    if (filter.actionType !== "all" && r.actionType !== filter.actionType) return false;
    if (filter.decision !== "all" && r.status !== filter.decision) return false;
    if (filter.rule === "matched" && !r.matchedRuleId) return false;
    if (filter.rule === "none" && r.matchedRuleId) return false;
    return true;
  });
}

/** 从请求列表提取实际出现过的 Provider / 动作类型，用于只展示相关筛选项。 */
export function auditFacets(requests: PermissionRequest[]): {
  providers: AgentKind[];
  actionTypes: PermissionActionType[];
} {
  const providers = new Set<AgentKind>();
  const actionTypes = new Set<PermissionActionType>();
  for (const r of requests) {
    providers.add(r.providerId);
    actionTypes.add(r.actionType);
  }
  return { providers: [...providers], actionTypes: [...actionTypes] };
}

/** 审计短摘要动词 (§9 第 21 条)：默认日志只显示请求/允许/拒绝这类简短文字。 */
export function auditVerb(request: PermissionRequest): string {
  switch (request.status) {
    case "pending":
      return "请求";
    case "approved":
      return "允许";
    case "denied":
      return "拒绝";
    case "cancelled":
      return "取消";
    case "expired":
      return "过期";
    default:
      return "请求";
  }
}

/** 一句话审计行：动词 + 动作类别 + 短标题。 */
export function auditLine(request: PermissionRequest): string {
  return `${auditVerb(request)} · ${actionCopy(request.actionType).label} · ${request.summary}`;
}

export function isActiveFilter(filter: AuditFilter): boolean {
  return (
    filter.provider !== "all" ||
    filter.actionType !== "all" ||
    filter.decision !== "all" ||
    filter.rule !== "all"
  );
}
