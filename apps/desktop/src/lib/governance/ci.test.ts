import { describe, expect, it } from "bun:test";
import type { CiCheck } from "@/generated/bindings";
import { ciCheckSummary, ciCounts, ciDuration } from "./ci";

const check = (overrides: Partial<CiCheck> = {}): CiCheck => ({
  name: "test",
  status: "passed",
  required: true,
  workflow: "CI",
  description: null,
  failureSummary: null,
  detailsUrl: "https://github.test/actions/runs/1/job/2",
  startedAt: "2026-07-20T10:00:00Z",
  completedAt: "2026-07-20T10:01:05Z",
  ...overrides,
});

describe("CI check presentation", () => {
  it("counts failures, pending checks and successes separately", () => {
    expect(ciCounts([
      check(),
      check({ status: "failed" }),
      check({ status: "cancelled" }),
      check({ status: "pending" }),
    ])).toEqual({ total: 4, passed: 1, failed: 2, pending: 1 });
  });

  it("prefers a redacted failure summary and never pretends missing detail exists", () => {
    expect(ciCheckSummary(check({ status: "failed", failureSummary: "typecheck failed" }))).toBe("typecheck failed");
    expect(ciCheckSummary(check({ status: "failed", detailsUrl: null }))).toContain("没有提供失败摘要");
  });

  it("formats completed duration without guessing pending time", () => {
    expect(ciDuration(check())).toBe("1 分 05 秒");
    expect(ciDuration(check({ completedAt: null }))).toBeNull();
  });
});
