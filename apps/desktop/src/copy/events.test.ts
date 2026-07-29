import { describe, expect, it } from "bun:test";
import { eventCopy, isSystemDetail } from "@/copy/events";

describe("Provider lifecycle timeline copy", () => {
  it("records start and failure as readable system timeline events", () => {
    const started = eventCopy("provider:started", { agent: "claude_code", role: "planner" });
    const failed = eventCopy("provider:failed", { agent: "claude_code", role: "planner", exit_code: 1 });
    expect(started.label).toContain("开始规划");
    expect(isSystemDetail(started)).toBe(true);
    expect(failed.label).toContain("规划失败");
    expect(failed.detail).toContain("退出代码 1");
  });

  it("surfaces the final blocked transition in the approval phase", () => {
    const blocked = eventCopy("task:blocked", { detail: "没有 Provider 能完成规划" });
    expect(blocked.phase).toBe("approval");
    expect(blocked.state).toBe("attention");
  });
});
