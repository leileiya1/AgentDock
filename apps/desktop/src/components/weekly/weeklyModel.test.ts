import { describe, expect, test } from "bun:test";
import type { Project, TaskSummary } from "@/generated/bindings";
import { gradeFor, trendText, weeklyReport } from "@/components/weekly/weeklyModel";

const now = new Date("2026-07-31T18:00:00Z");
const daysAgo = (d: number) => new Date(now.getTime() - d * 86_400_000).toISOString();
let n = 0;
const t = (over: Partial<TaskSummary>): TaskSummary =>
  ({ id: `t${++n}`, projectId: "p1", seq: n, title: "x", status: "MERGED", blockedReason: null, currentRevision: 1, developerAgent: "codex", reviewerAgent: "claude_code", updatedAt: daysAgo(1), ...over }) as TaskSummary;
const projects = [{ id: "p1", name: "acme" }, { id: "p2", name: "docs" }] as Project[];

describe("weeklyReport", () => {
  const tasks = [
    t({}),
    t({ currentRevision: 2 }),
    t({ developerAgent: "claude_code", projectId: "p2" }),
    t({ updatedAt: daysAgo(9) }), // last week
    t({ updatedAt: daysAgo(20) }), // older
    t({ status: "MERGE_CONFLICT", updatedAt: daysAgo(2) }),
    t({ status: "BLOCKED", updatedAt: daysAgo(30) }),
  ];
  const r = weeklyReport(tasks, projects, now);

  test("counts this week vs last week", () => {
    expect(r.merged).toBe(3);
    expect(r.mergedPrev).toBe(1);
    expect(r.trouble).toBe(1);
    expect(r.waiting).toBe(2); // conflict + blocked, regardless of age
  });

  test("rework rate, MVP and busiest project", () => {
    expect(r.reworkRate).toBeCloseTo(1 / 3);
    expect(r.mvp).toEqual({ agent: "codex", merged: 2 });
    expect(r.busiest).toEqual({ name: "acme", merged: 2 });
    expect(r.grade).toBe("A");
  });

  test("quiet week", () => {
    const q = weeklyReport([], projects, now);
    expect(q.grade).toBe("C");
    expect(q.mvp).toBeNull();
    expect(q.reworkRate).toBeNull();
    expect(q.comment).toContain("安静");
  });

  test("a C from rework says so", () => {
    const r2 = weeklyReport([t({ currentRevision: 3 })], projects, now);
    expect(r2.grade).toBe("C");
    expect(r2.comment).toContain("返工");
  });
});

test("grades", () => {
  expect(gradeFor(6, 0.1, 0)).toBe("A+");
  expect(gradeFor(6, 0.1, 1)).toBe("A");
  expect(gradeFor(3, 0.5, 2)).toBe("B");
  expect(gradeFor(3, 0.8, 0)).toBe("C");
});

test("trendText", () => {
  expect(trendText(5, 2)).toBe("比上周多 3");
  expect(trendText(1, 4)).toBe("比上周少 3");
  expect(trendText(2, 2)).toBe("和上周持平");
});
