import { describe, expect, test } from "bun:test";
import type { TaskStatus, TaskSummary } from "@/generated/bindings";
import { buildHomeTaskSections } from "./homeTasks";

function task(seq: number, status: TaskStatus): TaskSummary {
  return {
    id: `t${seq}`,
    projectId: "p1",
    seq,
    title: status,
    status,
    blockedReason: status === "BLOCKED" ? "run_failed" : null,
    currentRevision: 1,
    developerAgent: "qoder_cli",
    reviewerAgent: "grok_cli",
    updatedAt: `2026-07-${String(20 + seq).padStart(2, "0")}T00:00:00Z`,
  };
}

describe("buildHomeTaskSections", () => {
  test("counts only real execution states as running", () => {
    const sections = buildHomeTaskSections([
      task(1, "DRAFT"),
      task(2, "READY_FOR_DEVELOPMENT"),
      task(3, "DEVELOPING"),
      task(4, "REVIEWING"),
    ]);

    expect(sections.open).toHaveLength(4);
    expect(sections.running.map((item) => item.status)).toEqual(["REVIEWING", "DEVELOPING"]);
    expect(sections.counts.running).toBe(2);
  });

  test("keeps attention and terminal tasks in their factual groups", () => {
    const sections = buildHomeTaskSections([
      task(1, "WAITING_FOR_PLAN_APPROVAL"),
      task(2, "BLOCKED"),
      task(3, "MERGED"),
      task(4, "CANCELLED"),
    ]);

    expect(sections.counts).toEqual({ all: 4, attention: 2, running: 0, done: 2 });
    expect(sections.attention.map((item) => item.status)).toEqual(["BLOCKED", "WAITING_FOR_PLAN_APPROVAL"]);
    expect(sections.done.map((item) => item.status)).toEqual(["CANCELLED", "MERGED"]);
  });
});
