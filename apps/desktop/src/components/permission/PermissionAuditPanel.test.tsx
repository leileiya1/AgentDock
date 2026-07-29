import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { AuditRow } from "./PermissionAuditPanel";
import { makeRequest } from "@/lib/permission/fixtures";

describe("PermissionAuditPanel AuditRow (06 §9 第 21 条)", () => {
  it("默认展示简短「允许」文字，技术 JSON 收在高级详情里", () => {
    const html = renderToStaticMarkup(
      <AuditRow request={makeRequest({ status: "approved", actionType: "command_execute", summary: "运行测试" })} />
    );
    expect(html).toContain("允许");
    expect(html).toContain("运行测试");
    expect(html).toContain("高级详情");
    // 技术细节存在于 DOM（可展开），但收在 details 内。
    expect(html).toContain("operation_sha256");
    expect(html).toContain("<details");
  });

  it("拒绝记录显示拒绝动词", () => {
    const html = renderToStaticMarkup(<AuditRow request={makeRequest({ status: "denied" })} />);
    expect(html).toContain("拒绝");
  });
});
