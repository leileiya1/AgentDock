import { describe, expect, test } from "bun:test";
import type { TaskSummary } from "@/generated/bindings";
import { attentionItem, buildAttentionItems } from "./attention";

function task(status: TaskSummary["status"], blockedReason: TaskSummary["blockedReason"] = null): TaskSummary {
  return {
    id: status,
    projectId: "p1",
    seq: 1,
    title: "测试任务",
    status,
    blockedReason,
    currentRevision: 1,
    developerAgent: "qoder_cli",
    reviewerAgent: "grok_cli",
    updatedAt: "2026-07-29T00:00:00Z",
  };
}

describe("attentionItem", () => {
  test("separates permission requests from other recovery work", () => {
    expect(attentionItem(task("BLOCKED", "permission_required"))?.kind).toBe("permission");
    expect(attentionItem(task("BLOCKED", "run_failed"))?.kind).toBe("recovery");
  });

  test("classifies approvals, delivery and conflicts", () => {
    expect(attentionItem(task("WAITING_FOR_PLAN_APPROVAL"))?.kind).toBe("approval");
    expect(attentionItem(task("WAITING_FOR_HUMAN_APPROVAL"))?.kind).toBe("approval");
    expect(attentionItem(task("APPROVED"))?.kind).toBe("delivery");
    expect(attentionItem(task("MERGE_CONFLICT"))?.kind).toBe("conflict");
  });

  test("excludes work that does not need the user", () => {
    expect(buildAttentionItems([task("DEVELOPING"), task("MERGED")])).toEqual([]);
  });
});
