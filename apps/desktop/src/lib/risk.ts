import type { FileDiff, Review, ReviewIssue, Severity } from "@/generated/bindings";

/**
 * 安全与完整性风险模型 (05 §6.9). The backend already blocks on integrity violations;
 * this module makes the *reason* visible in the Diff 文件树、审查页 and 审批按钮旁边,
 * instead of leaving risk scattered inside a generic issue list.
 */

export type RiskKind = "control_plane" | "test_removed" | "permission_config" | "security_sensitive";

export interface RiskMeta {
  label: string;
  /** 为什么要盯着这一类改动。 */
  why: string;
  severity: "high" | "medium";
}

export const RISK_META: Record<RiskKind, RiskMeta> = {
  control_plane: {
    label: "控制面文件",
    why: "这些文件决定 Agent 的行为和权限，改动它们等于改规则本身。",
    severity: "high",
  },
  test_removed: {
    label: "删除测试",
    why: "删掉测试会让验证看起来通过，但实际覆盖变少。",
    severity: "high",
  },
  permission_config: {
    label: "权限 / 配置",
    why: "CI、依赖和构建配置会影响实际执行的命令。",
    severity: "medium",
  },
  security_sensitive: {
    label: "安全敏感",
    why: "认证、密钥和加密相关代码的改动需要额外确认。",
    severity: "medium",
  },
};

/** Agent 控制面：改了这些就等于改自己的行为边界。 */
const CONTROL_PLANE = [
  /(^|\/)CLAUDE\.md$/i,
  /(^|\/)AGENTS?\.md$/i,
  /(^|\/)\.agentflow\//,
  /(^|\/)\.cursor(rules)?(\/|$)/i,
  /(^|\/)\.claude\//,
];

const PERMISSION_CONFIG = [
  /(^|\/)\.github\/workflows\//,
  /(^|\/)\.gitlab-ci\.yml$/,
  /(^|\/)Dockerfile$/i,
  /(^|\/)docker-compose\.ya?ml$/i,
  /(^|\/)package\.json$/,
  /(^|\/)Cargo\.toml$/,
  /(^|\/)(pyproject|setup)\.(toml|py|cfg)$/,
  /(^|\/)\.env(\.|$)/,
  /(^|\/)(Makefile|justfile)$/i,
];

const SECURITY_SENSITIVE = [
  /(^|\/)(auth|authz|authentication|login|session|oauth)(\/|\.|$)/i,
  /(^|\/)(crypto|encryption|secrets?|credential|keychain|token)(\/|\.|$)/i,
  /\.(pem|key|p12|pfx|crt)$/i,
  /(^|\/)(security|permissions?|acl|policy)(\/|\.|$)/i,
];

const TEST_PATH = /(^|\/)(tests?|__tests__|spec|e2e)(\/|$)|\.(test|spec)\.[a-z]+$|_test\.[a-z]+$|(^|\/)test_[^/]+\.py$/i;

const matchesAny = (patterns: RegExp[], path: string) => patterns.some((pattern) => pattern.test(path));

export function isTestFile(path: string): boolean {
  return TEST_PATH.test(path);
}

/** 文件是否被整体删除：只有删除行、没有新增行。 */
export function isDeletion(file: FileDiff): boolean {
  return file.deletions > 0 && file.insertions === 0;
}

/**
 * A file can hit several categories at once (a deleted auth test is both), so return
 * all of them — collapsing to one label is how risk got lost in the first place.
 */
export function classifyFile(file: FileDiff): RiskKind[] {
  const path = file.path;
  const kinds: RiskKind[] = [];
  // 后端 flagged 是权威的控制面判定，前端规则只做补充。
  if (file.flagged || matchesAny(CONTROL_PLANE, path)) kinds.push("control_plane");
  if (isTestFile(path) && isDeletion(file)) kinds.push("test_removed");
  if (matchesAny(PERMISSION_CONFIG, path)) kinds.push("permission_config");
  if (matchesAny(SECURITY_SENSITIVE, path)) kinds.push("security_sensitive");
  return kinds;
}

export interface RiskSummary {
  /** path → 命中的风险类别。 */
  byFile: Map<string, RiskKind[]>;
  counts: Record<RiskKind, number>;
  /** 有任一高风险类别命中。 */
  hasHighRisk: boolean;
  /** 被删除的测试文件，用于「测试减少」说明 (05 §6.9)。 */
  removedTests: string[];
  /** 本轮改动里新增/修改的测试文件数，用来对比基线。 */
  touchedTests: string[];
}

export function summarizeRisk(files: FileDiff[]): RiskSummary {
  const byFile = new Map<string, RiskKind[]>();
  const counts: Record<RiskKind, number> = {
    control_plane: 0,
    test_removed: 0,
    permission_config: 0,
    security_sensitive: 0,
  };
  const removedTests: string[] = [];
  const touchedTests: string[] = [];

  for (const file of files) {
    const kinds = classifyFile(file);
    if (kinds.length > 0) {
      byFile.set(file.path, kinds);
      for (const kind of kinds) counts[kind] += 1;
    }
    if (isTestFile(file.path)) {
      if (isDeletion(file)) removedTests.push(file.path);
      else touchedTests.push(file.path);
    }
  }

  return {
    byFile,
    counts,
    hasHighRisk: counts.control_plane > 0 || counts.test_removed > 0,
    removedTests,
    touchedTests,
  };
}

/** 审批按钮旁边要持续显示的未解决高风险数量 (05 §6.9)。 */
export interface UnresolvedIssues {
  critical: number;
  high: number;
  total: number;
  /** 需要额外确认才能批准。 */
  requiresExtraConfirmation: boolean;
  items: ReviewIssue[];
}

const BLOCKING: Severity[] = ["critical", "high"];

export function unresolvedIssues(review: Review | null | undefined): UnresolvedIssues {
  const items = (review?.issues ?? []).filter((issue) => !issue.resolved);
  const critical = items.filter((issue) => issue.severity === "critical").length;
  const high = items.filter((issue) => issue.severity === "high").length;
  return {
    critical,
    high,
    total: items.length,
    requiresExtraConfirmation: critical + high > 0,
    items: items.filter((issue) => BLOCKING.includes(issue.severity)),
  };
}

/**
 * 高风险批准的确认文案必须具体到风险和文件，不能只问「确定吗」(05 §6.9 / §8)。
 */
export function highRiskConfirmation(risk: RiskSummary, issues: UnresolvedIssues): string[] {
  const lines: string[] = [];
  if (issues.critical > 0) lines.push(`${issues.critical} 个严重问题仍未解决`);
  if (issues.high > 0) lines.push(`${issues.high} 个高风险问题仍未解决`);
  if (risk.counts.control_plane > 0) {
    lines.push(`改动了 ${risk.counts.control_plane} 个控制面文件：${[...risk.byFile]
      .filter(([, kinds]) => kinds.includes("control_plane"))
      .map(([path]) => path)
      .slice(0, 3)
      .join("、")}`);
  }
  if (risk.removedTests.length > 0) {
    lines.push(`删除了 ${risk.removedTests.length} 个测试文件：${risk.removedTests.slice(0, 3).join("、")}`);
  }
  return lines;
}
