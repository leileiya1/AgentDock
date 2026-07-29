import type { BudgetEnforcement, BudgetUsage } from "@/generated/bindings";

/**
 * 预算视图模型 (05 §6.2). The single rule this module exists to enforce:
 * **未知用量绝不显示成 0**。`costUsd === null` 意味着「Provider 没有提供费用」，
 * 把它渲染成 `$0` 会让用户以为这次运行免费。
 */

export type BudgetKey = "tokens" | "cost" | "time";

/** 70% / 90% / 100% 分级提醒，只有 100% 才是明确阻断 (05 §6.2)。 */
export type BudgetLevel = "ok" | "notice" | "warn" | "blocked";

export interface BudgetMetric {
  key: BudgetKey;
  label: string;
  /** 是否拿得到用量。false → 显示「未知」而不是 0。 */
  known: boolean;
  /** 有多少次运行没能提供用量。 */
  unknownRuns: number;
  /** 已结算用量；未知时为 null。 */
  settled: number | null;
  /** 已预留但尚未结算的用量 (05 §6.2)。 */
  reserved: number;
  limit: number | null;
  enforcement: BudgetEnforcement;
  enforcementLabel: string;
  /** 已结算 / 已预留 / 剩余三段进度，百分比。limit 为空时全 0。 */
  settledPercent: number;
  reservedPercent: number;
  level: BudgetLevel;
  /** 主显示文字，例如 `32,840` / `未知` / `8 分 00 秒`。 */
  display: string;
  limitDisplay: string;
  /** 未知用量的解释文案。 */
  note: string | null;
}

export interface BudgetView {
  metrics: BudgetMetric[];
  /** 任务头部的一行预算健康摘要 (05 §6.2)。 */
  headline: string;
  level: BudgetLevel;
  exceeded: boolean;
  /** 任一维度用量未知——调整预算时不能用 0 自动建议新上限。 */
  hasUnknown: boolean;
}

const ENFORCEMENT_LABEL: Record<BudgetEnforcement, string> = {
  hard: "硬限制",
  soft: "仅提醒",
  unavailable: "Provider 不提供用量",
};

const LEVEL_RANK: Record<BudgetLevel, number> = { ok: 0, notice: 1, warn: 2, blocked: 3 };

function levelFor(used: number, limit: number | null): BudgetLevel {
  if (limit == null || limit <= 0) return "ok";
  const ratio = used / limit;
  if (ratio >= 1) return "blocked";
  if (ratio >= 0.9) return "warn";
  if (ratio >= 0.7) return "notice";
  return "ok";
}

export function formatTokens(value: number): string {
  return value.toLocaleString("zh-CN");
}

export function formatUsd(value: number): string {
  return `$${value.toFixed(value >= 1 ? 2 : 4)}`;
}

export function formatDuration(seconds: number): string {
  if (seconds < 60) return `${Math.round(seconds)} 秒`;
  const mins = Math.floor(seconds / 60);
  if (mins < 60) return `${mins} 分 ${String(Math.round(seconds % 60)).padStart(2, "0")} 秒`;
  return `${Math.floor(mins / 60)} 小时 ${String(mins % 60).padStart(2, "0")} 分`;
}

interface MetricInput {
  key: BudgetKey;
  label: string;
  known: boolean;
  unknownRuns: number;
  settled: number | null;
  reserved: number;
  limit: number | null;
  enforcement: BudgetEnforcement;
  format: (value: number) => string;
}

function metric(input: MetricInput): BudgetMetric {
  const { known, settled, reserved, limit, format } = input;
  const total = (settled ?? 0) + reserved;
  const percent = (value: number) => (limit && limit > 0 ? Math.min(100, (value / limit) * 100) : 0);

  return {
    key: input.key,
    label: input.label,
    known,
    unknownRuns: input.unknownRuns,
    settled,
    reserved,
    limit,
    enforcement: input.enforcement,
    enforcementLabel: ENFORCEMENT_LABEL[input.enforcement],
    // 进度条区分已结算与已预留，不只画一段 used (05 §6.2)。
    settledPercent: known ? percent(settled ?? 0) : 0,
    reservedPercent: known ? Math.max(0, percent(total) - percent(settled ?? 0)) : 0,
    level: known ? levelFor(total, limit) : "ok",
    display: known && settled != null ? format(settled) : "未知",
    limitDisplay: limit == null ? "不限" : format(limit),
    note: noteFor(input),
  };
}

function noteFor({ known, unknownRuns, reserved, enforcement, format }: MetricInput): string | null {
  const parts: string[] = [];
  if (enforcement === "unavailable") parts.push("Provider 不提供这项用量，无法作为硬限制");
  if (!known) parts.push(unknownRuns > 0 ? `${unknownRuns} 次运行未提供用量` : "还没有可统计的用量");
  else if (unknownRuns > 0) parts.push(`另有 ${unknownRuns} 次运行未提供用量，实际值可能更高`);
  if (reserved > 0) parts.push(`含已预留 ${format(reserved)}，运行结束后结算`);
  return parts.length > 0 ? parts.join("；") : null;
}

export function budgetView(usage: BudgetUsage): BudgetView {
  const metrics: BudgetMetric[] = [
    metric({
      key: "tokens",
      label: "Token",
      known: usage.tokensKnown,
      unknownRuns: usage.unknownTokenRuns,
      settled: usage.tokensKnown ? usage.tokensUsed : null,
      reserved: usage.tokensReserved,
      limit: usage.tokenBudget,
      enforcement: usage.tokenEnforcement,
      format: formatTokens,
    }),
    metric({
      key: "cost",
      label: "费用",
      // costUsd 为 null 时既不是 0 也不是「免费」，必须显示未知。
      known: usage.costKnown && usage.costUsd != null,
      unknownRuns: usage.unknownCostRuns,
      settled: usage.costUsd,
      reserved: usage.costReservedUsd ?? 0,
      limit: usage.costBudgetUsd,
      enforcement: usage.costEnforcement,
      format: formatUsd,
    }),
    metric({
      key: "time",
      label: "运行时间",
      known: true,
      unknownRuns: 0,
      settled: usage.timeUsedSecs,
      reserved: 0,
      limit: usage.timeBudgetSecs,
      // 时间由本地时钟统计，永远可得，因此是硬限制。
      enforcement: "hard",
      format: formatDuration,
    }),
  ];

  const level = metrics.reduce<BudgetLevel>(
    (acc, m) => (LEVEL_RANK[m.level] > LEVEL_RANK[acc] ? m.level : acc),
    "ok"
  );
  const hasUnknown = metrics.some((m) => !m.known);

  return {
    metrics,
    headline: headlineFor(metrics, usage.exceeded, level, hasUnknown),
    level: usage.exceeded ? "blocked" : level,
    exceeded: usage.exceeded,
    hasUnknown,
  };
}

function headlineFor(
  metrics: BudgetMetric[],
  exceeded: boolean,
  level: BudgetLevel,
  hasUnknown: boolean
): string {
  if (exceeded) return "预算已用完，任务已停在检查点";
  const limited = metrics.filter((m) => m.limit != null && m.known);
  const tightest = limited
    .slice()
    .sort((a, b) => b.settledPercent + b.reservedPercent - (a.settledPercent + a.reservedPercent))[0];

  if (!tightest) return hasUnknown ? "预算用量部分未知" : "未设置预算上限";

  const remaining = Math.max(0, 100 - tightest.settledPercent - tightest.reservedPercent);
  const base = `${tightest.label}剩余约 ${Math.round(remaining)}%`;
  if (hasUnknown) return `${base}（部分用量未知）`;
  if (level === "warn") return `${base}，接近上限`;
  if (level === "notice") return `${base}`;
  return base;
}

/**
 * 调整预算弹窗的建议值 (05 §6.2)。用量未知时不能用 0 推算新上限——
 * 返回 null，让 UI 要求用户自己填。
 */
export function suggestedLimit(metric: BudgetMetric, multiplier = 2): number | null {
  if (!metric.known || metric.settled == null) return null;
  const base = Math.max(metric.settled + metric.reserved, metric.limit ?? 0);
  if (base <= 0) return null;
  return Math.ceil(base * multiplier);
}
