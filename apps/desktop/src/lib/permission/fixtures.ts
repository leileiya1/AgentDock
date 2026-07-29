import type {
  PermissionDecision,
  PermissionRequest,
  PermissionRule,
} from "@/generated/bindings";

/**
 * 权限相关测试与开发夹具。仅被测试和开发预览引用，不进入生产运行路径。
 * 构造符合后端 DTO 形状的样本请求/规则，避免每个测试重复长对象。
 */

const CWD = "/Users/dev/AgentFlow/worktrees/t12";

export function makeRequest(over: Partial<PermissionRequest> = {}): PermissionRequest {
  return {
    id: "req-1",
    projectId: "p1",
    taskId: "t12",
    revision: 2,
    runId: "run-1",
    providerId: "claude",
    role: "developer",
    actionType: "command_execute",
    summary: "运行测试命令 bun test",
    reason: "开发 Agent 需要运行工作树内的测试来验证改动。",
    operation: {
      argv: ["bun", "test", "src/lib/permission"],
      cwd: CWD,
      paths: [],
      networkDomains: [],
      environmentNames: [],
    },
    riskLevel: "medium",
    grantable: true,
    operationSha256: "op-sha-aaa",
    policySha256: "policy-sha-aaa",
    status: "pending",
    matchedRuleId: null,
    requestCount: 1,
    requestedAt: "2026-07-21T10:00:00Z",
    expiresAt: "2026-07-21T10:05:00Z",
    decidedAt: null,
    providerResumeToken: null,
    ...over,
  };
}

export function makeRule(over: Partial<PermissionRule> = {}): PermissionRule {
  return {
    id: "rule-1",
    projectId: "p1",
    projectIdentity: "identity-aaa",
    providerId: "claude",
    role: "developer",
    actionType: "command_execute",
    operation: {
      argv: ["bun", "test"],
      cwd: CWD,
      paths: [],
      networkDomains: [],
      environmentNames: [],
    },
    ruleSha256: "rule-sha-aaa",
    enabled: true,
    createdBy: "human",
    createdAt: "2026-07-21T09:00:00Z",
    expiresAt: null,
    lastMatchedAt: "2026-07-21T09:30:00Z",
    revokedAt: null,
    ...over,
  };
}

export function makeDecision(over: Partial<PermissionDecision> = {}): PermissionDecision {
  return {
    id: "dec-1",
    requestId: "req-1",
    operationSha256: "op-sha-aaa",
    policySha256: "policy-sha-aaa",
    decision: "approve",
    scope: "once",
    expiresAt: "2026-07-21T10:05:00Z",
    approvedBy: "human",
    guidance: null,
    createdAt: "2026-07-21T10:01:00Z",
    consumedAt: null,
    ...over,
  };
}

export const FIXTURE_CWD = CWD;
