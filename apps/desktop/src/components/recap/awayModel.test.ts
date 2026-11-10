import { describe, expect, test } from "bun:test";
import type { Project, TaskSummary } from "@/generated/bindings";
import { AWAY_MS, awayLabel, buildRecap } from "@/components/recap/awayModel";

const now = Date.parse("2026-07-30T12:00:00Z");
const projects = [{ id: "p1", name: "acme" }] as Project[];
const task = (id: string, status: TaskSummary["status"], minsAgo: number) =>
  ({ id, projectId: "p1", seq: 1, title: id, status, updatedAt: new Date(now - minsAgo * 60_000).toISOString() }) as TaskSummary;

describe("buildRecap", () => {
  const tasks = [task("m", "MERGED", 30), task("w", "BLOCKED", 20), task("c", "MERGE_CONFLICT", 10), task("r", "REVIEWING", 5), task("old", "MERGED", 300), task("x", "CANCELLED", 5)];

  test("nothing on first launch or a short break", () => {
    expect(buildRecap(tasks, projects, null, now)).toBeNull();
    expect(buildRecap(tasks, projects, now - AWAY_MS + 1, now)).toBeNull();
  });

  test("groups only tasks touched while away; trouble comes first after merges", () => {
    const r = buildRecap(tasks, projects, now - 120 * 60_000, now)!;
    expect(r.counts).toEqual({ merged: 1, waiting: 1, trouble: 1, moving: 1 });
    expect(r.items.map((i) => i.taskId)).toEqual(["m", "c", "w", "r"]);
    expect(r.items[0].projectName).toBe("acme");
    expect(r.headline).toContain("岔子");
  });

  test("quiet absence yields no recap", () => {
    expect(buildRecap([task("old", "MERGED", 300)], projects, now - 120 * 60_000, now)).toBeNull();
  });
});

test("awayLabel", () => {
  expect(awayLabel(25 * 60_000)).toBe("25 分钟");
  expect(awayLabel(192 * 60_000)).toBe("3 小时 12 分");
  expect(awayLabel(26 * 3_600_000)).toBe("1 天 2 小时");
});
