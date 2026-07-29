import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ExecutionNodeDiagnostics } from "./ExecutionNodeDiagnostics";

describe("ExecutionNodeDiagnostics", () => {
  it("shows a precise failed step and its recovery instead of one offline string", () => {
    const html = renderToStaticMarkup(<ExecutionNodeDiagnostics diagnostics={[
      { step: "dns", status: "passed", blocking: true, summary: "解析到 1 个地址", detail: "10.0.0.1:22", durationMs: 2, checkedAt: "now" },
      { step: "tcp", status: "failed", blocking: true, summary: "SSH TCP 端口不可达", detail: "connection refused", durationMs: 4, checkedAt: "now" },
      { step: "ssh_authentication", status: "skipped", blocking: true, summary: "未执行", detail: "TCP 失败", durationMs: 0, checkedAt: "now" },
    ]} />);
    expect(html).toContain("TCP 端口");
    expect(html).toContain("connection refused");
    expect(html).toContain("防火墙");
    expect(html).toContain("未执行");
  });
});
