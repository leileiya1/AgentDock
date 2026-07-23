import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { RuleRow } from "./PermissionRulesPanel";
import { makeRule } from "@/lib/permission/fixtures";

describe("PermissionRulesPanel RuleRow (06 §9 第 18/19 条)", () => {
  it("生效规则展示命中/到期并提供撤销", () => {
    const html = renderToStaticMarkup(
      <RuleRow rule={makeRule({ actionType: "command_execute", operation: { argv: ["bun", "test"], cwd: "/w", paths: [], networkDomains: [], environmentNames: [] } })} onRevoke={() => {}} />
    );
    expect(html).toContain("执行命令");
    expect(html).toContain("最近命中");
    expect(html).toContain("到期");
    expect(html).toContain("撤销");
    expect(html).toContain("生效中");
  });

  it("已撤销规则不再提供撤销按钮", () => {
    const html = renderToStaticMarkup(
      <RuleRow rule={makeRule({ revokedAt: "2026-07-21T09:00:00Z", enabled: false })} onRevoke={() => {}} />
    );
    expect(html).toContain("已撤销");
    // 撤销按钮只在生效状态出现；已撤销规则的行内不含独立的「撤销」按钮文本节点。
    expect(html).not.toContain(">撤销<");
  });

  it("展示精确域名而非通配", () => {
    const html = renderToStaticMarkup(
      <RuleRow
        rule={makeRule({ actionType: "dependency_install", operation: { argv: ["bun", "add"], cwd: "/w", paths: [], networkDomains: ["registry.npmjs.org:443"], environmentNames: [] } })}
        onRevoke={() => {}}
      />
    );
    expect(html).toContain("registry.npmjs.org:443");
    expect(html).not.toContain("*");
  });
});
