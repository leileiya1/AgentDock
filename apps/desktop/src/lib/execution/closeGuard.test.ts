import { describe, expect, it } from "bun:test";
import type { TaskSummary } from "@/generated/bindings";
import { countActiveTasks } from "./closeGuard";

function task(status: TaskSummary["status"]): TaskSummary {
  return {
    id: "t",
    projectId: "p",
    seq: 1,
    title: "t",
    status,
    blockedReason: null,
    currentRevision: 1,
    developerAgent: "claude_code",
    reviewerAgent: "codex",
    updatedAt: "2026-07-21T00:00:00Z",
  };
}

describe("countActiveTasks", () => {
  it("counts running tasks across every cached project list", () => {
    const lists = [
      [task("DEVELOPING"), task("WAITING_FOR_HUMAN_APPROVAL")],
      [task("REVIEWING"), task("MERGING")],
    ];
    expect(countActiveTasks(lists)).toBe(3);
  });

  it("ignores settled tasks and empty/undefined lists", () => {
    const lists = [
      [task("BLOCKED"), task("MERGED"), task("DRAFT")],
      undefined,
      [],
    ];
    expect(countActiveTasks(lists)).toBe(0);
  });
});
