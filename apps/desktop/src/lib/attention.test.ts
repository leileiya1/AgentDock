import { describe, expect, test } from "bun:test";
import type { TaskSummary } from "@/generated/bindings";
import { attentionItem, buildAttentionItems, groupAttentionItems } from "./attention";

function task(
  status: TaskSummary["status"],
  blockedReason: TaskSummary["blockedReason"] = null,
  seq = 1,
  updatedAt = "2026-07-29T00:00:00Z",
): TaskSummary {
  return {
    id: `${status}-${seq}`,
    projectId: "p1",
    seq,
    title: "测试任务",
    status,
    blockedReason,
    currentRevision: 1,
    developerAgent: "qoder_cli",
    reviewerAgent: "grok_cli",
    updatedAt,
  };
}

describe("attentionItem", () => {
  test("uses specific reasons and actions for user decisions", () => {
    expect(attentionItem(task("WAITING_FOR_PLAN_APPROVAL"))).toMatchObject({
      reason: "计划已生成，等待批准",
      action: "审阅计划",
      severity: "normal",
    });
    expect(attentionItem(task("WAITING_FOR_HUMAN_APPROVAL"))?.action).toBe("核对验收");
    expect(attentionItem(task("APPROVED"))?.action).toBe("完成交付");
  });

  test("separates permission, recovery, quality and conflict actions", () => {
    expect(attentionItem(task("BLOCKED", "permission_required"))).toMatchObject({ kind: "permission", action: "核对权限" });
    expect(attentionItem(task("BLOCKED", "auth_expired"))).toMatchObject({ severity: "medium", action: "恢复任务" });
    expect(attentionItem(task("BLOCKED", "quality_gate"))).toMatchObject({ severity: "high", action: "查看门禁" });
    expect(attentionItem(task("MERGE_CONFLICT"))).toMatchObject({ severity: "high", action: "处理冲突" });
  });

  test("excludes work that does not need the user", () => {
    expect(buildAttentionItems([task("DEVELOPING"), task("MERGED")])).toEqual([]);
  });
});

describe("buildAttentionItems", () => {
  test("sorts by product impact, then older update time, then task sequence", () => {
    const items = buildAttentionItems([
      task("WAITING_FOR_PLAN_APPROVAL", null, 5, "2026-07-29T02:00:00Z"),
      task("BLOCKED", "auth_expired", 4, "2026-07-29T03:00:00Z"),
      task("BLOCKED", "quality_gate", 3, "2026-07-29T04:00:00Z"),
      task("MERGE_CONFLICT", null, 2, "2026-07-29T01:00:00Z"),
      task("BLOCKED", "quality_gate", 1, "2026-07-29T04:00:00Z"),
    ]);

    expect(items.map((item) => item.task.seq)).toEqual([2, 1, 3, 4, 5]);
  });

  test("groups repeated reasons without inventing batch actions", () => {
    const groups = groupAttentionItems(buildAttentionItems([
      task("WAITING_FOR_PLAN_APPROVAL", null, 1),
      task("WAITING_FOR_PLAN_APPROVAL", null, 2),
      task("BLOCKED", "permission_required", 3),
    ]));

    expect(groups.find((group) => group.key === "plan-approval")?.items).toHaveLength(2);
    expect(groups.find((group) => group.key === "permission_required")?.items).toHaveLength(1);
  });
});
