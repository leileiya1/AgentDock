import { describe, expect, it } from "bun:test";
import type { BudgetUsage } from "@/generated/bindings";
import { budgetView, suggestedLimit } from "./budget";

const usage = (over: Partial<BudgetUsage> = {}): BudgetUsage => ({
  tokensUsed: 100_000,
  costUsd: 1.25,
  timeUsedSecs: 600,
  tokenBudget: 500_000,
  costBudgetUsd: 25,
  timeBudgetSecs: 7_200,
  tokensKnown: true,
  costKnown: true,
  unknownTokenRuns: 0,
  unknownCostRuns: 0,
  tokensReserved: 0,
  costReservedUsd: 0,
  tokenEnforcement: "hard",
  costEnforcement: "hard",
  exceeded: false,
  ...over,
});

const pick = (u: BudgetUsage, key: "tokens" | "cost" | "time") =>
  budgetView(u).metrics.find((m) => m.key === key)!;

describe("预算 · 未知用量 (05 §6.2)", () => {
  it("费用未知时显示「未知」而不是 $0", () => {
    const cost = pick(usage({ costUsd: null, costKnown: false, unknownCostRuns: 3 }), "cost");

    expect(cost.display).toBe("未知");
    expect(cost.display).not.toBe("$0.0000");
    expect(cost.note).toContain("3 次运行未提供用量");
  });

  it("未知用量不画进度，避免暗示已用 0", () => {
    const cost = pick(usage({ costUsd: null, costKnown: false }), "cost");

    expect(cost.settledPercent).toBe(0);
    expect(cost.reservedPercent).toBe(0);
    expect(cost.level).toBe("ok");
  });

  it("Token 可统计而费用未知时，摘要要说明部分未知", () => {
    const view = budgetView(usage({ costUsd: null, costKnown: false, unknownCostRuns: 2 }));

    expect(view.hasUnknown).toBe(true);
    expect(view.headline).toContain("部分用量未知");
  });

  it("部分运行未提供用量时提示实际值可能更高", () => {
    const tokens = pick(usage({ unknownTokenRuns: 1 }), "tokens");
    expect(tokens.note).toContain("实际值可能更高");
  });
});

describe("预算 · 已预留 (05 §6.2)", () => {
  it("已预留与已结算分成两段展示", () => {
    const tokens = pick(usage({ tokensUsed: 100_000, tokensReserved: 50_000 }), "tokens");

    expect(tokens.settledPercent).toBe(20);
    expect(tokens.reservedPercent).toBe(10);
    expect(tokens.note).toContain("含已预留");
  });

  it("预留计入阈值判定", () => {
    const tokens = pick(usage({ tokensUsed: 300_000, tokensReserved: 100_000 }), "tokens");
    expect(tokens.level).toBe("notice"); // 80%
  });
});

describe("预算 · 分级与阻断 (05 §6.2)", () => {
  const thresholds: Array<[number, string]> = [
    [300_000, "ok"],
    [360_000, "notice"],
    [455_000, "warn"],
    [500_000, "blocked"],
  ];
  for (const [used, expected] of thresholds) {
    it(`用量 ${used} → ${expected}`, () => {
      expect(pick(usage({ tokensUsed: used }), "tokens").level).toBe(expected as never);
    });
  }

  it("只有真正超限才是阻断", () => {
    expect(budgetView(usage({ tokensUsed: 455_000 })).level).toBe("warn");
    expect(budgetView(usage({ exceeded: true })).level).toBe("blocked");
    expect(budgetView(usage({ exceeded: true })).headline).toBe("预算已用完，任务已停在检查点");
  });
});

describe("预算 · 执行方式文案 (05 §6.2)", () => {
  it("hard / soft / unavailable 都翻译成用户语言", () => {
    const view = budgetView(usage({ tokenEnforcement: "soft", costEnforcement: "unavailable", costKnown: false, costUsd: null }));

    expect(view.metrics.find((m) => m.key === "tokens")!.enforcementLabel).toBe("仅提醒");
    const cost = view.metrics.find((m) => m.key === "cost")!;
    expect(cost.enforcementLabel).toBe("Provider 不提供用量");
    expect(cost.note).toContain("无法作为硬限制");
  });
});

describe("预算 · 调整上限建议 (05 §6.2)", () => {
  it("用量未知时不给建议值，避免用 0 推算", () => {
    const cost = pick(usage({ costUsd: null, costKnown: false }), "cost");
    expect(suggestedLimit(cost)).toBeNull();
  });

  it("用量已知时基于已结算 + 已预留给建议", () => {
    const tokens = pick(usage({ tokensUsed: 400_000, tokensReserved: 100_000 }), "tokens");
    expect(suggestedLimit(tokens)).toBe(1_000_000);
  });
});
