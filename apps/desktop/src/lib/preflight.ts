import type {
  AgentKind,
  PreflightRole,
  ProjectSettings,
  ProviderDescriptor,
  ProviderPreflightArgs,
  ProviderReadiness,
  RoleReadiness,
  TaskPreflightReport,
} from "@/generated/bindings";
import { agentLabel, isApiAgent } from "@/copy/agents";

/** 角色的人话名称：Preflight 报告里只有 developer/reviewer 两种角色。 */
export function preflightRoleLabel(role: PreflightRole): string {
  return role === "developer" ? "开发" : "审查";
}

export interface PreflightProviderView {
  key: string;
  label: string;
  available: boolean;
  problem: string | null;
}

export interface PreflightRoleView {
  role: PreflightRole;
  label: string;
  ready: boolean;
  primaryLabel: string;
  providers: PreflightProviderView[];
}

export interface PreflightView {
  ready: boolean;
  roles: PreflightRoleView[];
  /** 没有任何可运行 Provider 的角色——正是它们挡住了启动。 */
  blockingRoles: PreflightRoleView[];
}

export interface PendingPreflightRole {
  role: PreflightRole;
  label: string;
  providers: Array<{ key: AgentKind; label: string }>;
}

function unique(values: AgentKind[]): AgentKind[] {
  return values.filter((value, index) => values.indexOf(value) === index);
}

/**
 * Mirrors the backend's candidate selection closely enough to name the real chains
 * while the concurrent probes are in flight. This is not a fake percentage: every
 * listed Provider remains "检测中" until the authoritative report comes back.
 */
export function pendingPreflightRoles(
  args: ProviderPreflightArgs,
  settings: ProjectSettings | undefined,
  catalog: ProviderDescriptor[],
): PendingPreflightRole[] {
  const descriptor = (kind: AgentKind) => catalog.find((provider) => provider.id === kind);
  const needsEgress = (kind: AgentKind) => {
    const provider = descriptor(kind);
    return isApiAgent(kind)
      || (!!provider && provider.executionLocation !== "local")
      || (!!provider && provider.dataEgress !== "none")
      || (provider?.permissions.networkDomains?.length ?? 0) > 0;
  };
  const allowed = (kind: AgentKind) => args.allowApiEgress || !needsEgress(kind);
  const label = (kind: AgentKind) => descriptor(kind)?.displayName || agentLabel(kind);

  const developer = unique([
    args.developerAgent,
    ...(settings?.developerFallbacks ?? []),
  ]).filter((kind) => !isApiAgent(kind))
    .filter((kind) => !args.requirePlanApproval || descriptor(kind)?.capabilities.planning !== false)
    .filter(allowed);

  const reviewer = unique([
    args.reviewerAgent,
    ...(isApiAgent(args.reviewerAgent) && settings?.apiFallbackProvider
      ? [settings.apiFallbackProvider]
      : []),
    ...(settings?.reviewerFallbacks ?? []),
  ]).filter(allowed);

  return [
    {
      role: "developer",
      label: preflightRoleLabel("developer"),
      providers: developer.map((kind) => ({ key: kind, label: label(kind) })),
    },
    {
      role: "reviewer",
      label: preflightRoleLabel("reviewer"),
      providers: reviewer.map((kind) => ({ key: kind, label: label(kind) })),
    },
  ];
}

function providerView(entry: ProviderReadiness): PreflightProviderView {
  return {
    key: entry.provider,
    label: entry.displayName || agentLabel(entry.provider),
    available: entry.available,
    problem: entry.problem ?? null,
  };
}

function roleView(role: RoleReadiness): PreflightRoleView {
  return {
    role: role.role,
    label: preflightRoleLabel(role.role),
    ready: role.ready,
    primaryLabel: agentLabel(role.primary),
    providers: role.chain.map(providerView),
  };
}

/**
 * 把后端 Preflight 报告压成 UI 视图模型：既保留每个角色的完整降级链和逐项原因，
 * 又单独拎出「挡住启动」的角色，供阻断对话框直接展示（P0-01/P0-02）。
 */
export function summarizePreflight(report: TaskPreflightReport): PreflightView {
  const roles = report.roles.map(roleView);
  return {
    ready: report.ready,
    roles,
    blockingRoles: roles.filter((role) => !role.ready),
  };
}

/** 一句话汇总为什么不能启动，用于 toast/无障碍朗读。 */
export function preflightBlockLine(view: PreflightView): string {
  if (view.ready) return "环境就绪，可以启动。";
  const names = view.blockingRoles.map((role) => role.label).join("、");
  return `${names}角色没有可运行的 Provider，修复后即可从这里启动。`;
}
