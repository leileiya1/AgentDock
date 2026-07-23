import { describe, expect, it } from "bun:test";
import { acceptanceStatus } from "./acceptance";

describe("结构化验收条件证据映射（P2-02）", () => {
  it("构建和测试只依据真实验证结果", () => {
    expect(acceptanceStatus("build", "passed", null)).toBe("passed");
    expect(acceptanceStatus("test", "failed", null)).toBe("failed");
    expect(acceptanceStatus("test", "skipped", null)).toBe("unverified");
    expect(acceptanceStatus("build", "none", "pass")).toBe("pending");
  });

  it("行为条件依据独立审查，人工条件必须明确确认", () => {
    expect(acceptanceStatus("behavior", "passed", "pass")).toBe("passed");
    expect(acceptanceStatus("behavior", "passed", "request_changes")).toBe("failed");
    expect(acceptanceStatus("manual", "passed", "pass")).toBe("manual");
    expect(acceptanceStatus("manual", "passed", "pass", true)).toBe("passed");
  });
});
