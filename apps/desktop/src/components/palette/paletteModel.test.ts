import { describe, expect, test } from "bun:test";
import type { Project, TaskSummary } from "@/generated/bindings";
import { buildItems, fuzzyScore, rankItems } from "@/components/palette/paletteModel";
import { chimeFor } from "@/hooks/useGlobalEvents";

const project = (id: string, name: string) => ({ id, name }) as Project;
const task = (id: string, projectId: string, title: string, status: TaskSummary["status"], updatedAt = "2026-07-30T10:00:00Z") =>
  ({ id, projectId, seq: 7, title, status, updatedAt }) as TaskSummary;

describe("fuzzyScore", () => {
  test("substring beats scattered subsequence; misses are -1", () => {
    expect(fuzzyScore("dock", "agentdock-desktop")).toBeGreaterThan(fuzzyScore("adk", "agentdock-desktop"));
    expect(fuzzyScore("adk", "agentdock-desktop")).toBeGreaterThan(0);
    expect(fuzzyScore("zzz", "agentdock")).toBe(-1);
  });
  test("prefix match ranks above a mid-word match", () => {
    expect(fuzzyScore("ag", "agent")).toBeGreaterThan(fuzzyScore("ag", "flag"));
  });
});

describe("palette items", () => {
  const items = buildItems({
    projects: [project("p1", "agentdock"), project("p2", "docs-site")],
    tasks: [task("t1", "p1", "修登录按钮", "MERGED"), task("t2", "p2", "写安装指南", "WAITING_FOR_HUMAN_APPROVAL")],
    currentProjectId: "p1",
    chimeOn: false,
  });

  test("tasks link across projects and carry the project name", () => {
    const t2 = items.find((i) => i.id === "t:t2")!;
    expect(t2.to).toBe("/p/p2/t/t2");
    expect(t2.hint).toContain("docs-site");
    expect(t2.urgent).toBe(true);
  });

  test("new-task action only appears inside a project", () => {
    expect(items.some((i) => i.run === "newTask")).toBe(true);
    expect(buildItems({ projects: [], tasks: [], chimeOn: true }).some((i) => i.run === "newTask")).toBe(false);
  });

  test("empty query lists waiting tasks before other tasks", () => {
    const ranked = rankItems(items, "");
    expect(ranked.findIndex((i) => i.id === "t:t2")).toBeLessThan(ranked.findIndex((i) => i.id === "t:t1"));
  });

  test("searching by project name finds its tasks", () => {
    expect(rankItems(items, "docs").map((i) => i.id)).toContain("t:t2");
  });

  test("waiting tasks that do not match stay out", () => {
    expect(rankItems(items, "agentdock").map((i) => i.id)).not.toContain("t:t2");
  });
});

describe("chimeFor", () => {
  test("rings on merge, on entering attention, and on trouble; not on repeats", () => {
    expect(chimeFor("MERGING", "MERGED")).toBe("done");
    expect(chimeFor("REVIEWING", "WAITING_FOR_HUMAN_APPROVAL")).toBe("attention");
    expect(chimeFor("BLOCKED", "WAITING_FOR_HUMAN_APPROVAL")).toBeNull();
    expect(chimeFor("MERGING", "MERGE_CONFLICT")).toBe("failed");
    expect(chimeFor("MERGED", "MERGED")).toBeNull();
    expect(chimeFor("REVIEWING", "REVISING")).toBeNull();
  });
});
