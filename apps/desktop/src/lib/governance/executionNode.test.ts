import { describe, expect, it } from "bun:test";
import { NODE_RECOVERY, NODE_STEP_LABEL, nodeStatusCopy } from "./executionNode";

describe("execution node diagnostics copy", () => {
  it("keeps transport, authentication and tool failures distinguishable", () => {
    expect(NODE_STEP_LABEL.tcp).toBe("TCP 端口");
    expect(NODE_STEP_LABEL.ssh_authentication).toBe("SSH 认证");
    expect(NODE_RECOVERY.work_root).toContain("可写目录");
    expect(nodeStatusCopy("offline")).toBe("不可用");
  });
});
