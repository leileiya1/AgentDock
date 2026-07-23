import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { PermissionOperationView } from "./PermissionOperationView";
import { PermissionRequestCard } from "./PermissionRequestCard";
import { PermissionRuleForm } from "./PermissionRuleForm";
import { FIXTURE_CWD, makeRequest } from "@/lib/permission/fixtures";

describe("PermissionOperationView (06 §9 第 6/7/8/9 条)", () => {
  it("命令以等宽 argv + 人话说明展示", () => {
    const html = renderToStaticMarkup(
      <PermissionOperationView request={makeRequest({ operation: { argv: ["bun", "add", "zod"], cwd: FIXTURE_CWD, paths: [], networkDomains: [], environmentNames: [] } })} />
    );
    expect(html).toContain("执行的命令");
    expect(html).toContain("bun add zod");
    expect(html).toContain("bun add"); // 人话说明
  });

  it("工作树内路径显示相对路径，工作树外显示完整位置并标记高风险", () => {
    const html = renderToStaticMarkup(
      <PermissionOperationView
        request={makeRequest({
          operation: {
            argv: [],
            cwd: FIXTURE_CWD,
            paths: [
              { path: `${FIXTURE_CWD}/src/app.ts`, access: "write", outsideWorktree: false },
              { path: "/etc/hosts", access: "read", outsideWorktree: true },
            ],
            networkDomains: [],
            environmentNames: [],
          },
        })}
      />
    );
    expect(html).toContain("src/app.ts");
    expect(html).toContain("/etc/hosts");
    expect(html).toContain("工作树外 · 高风险");
  });

  it("网络显示精确域名端口协议，并标明是 Agent 命令访问网络而非模型外发", () => {
    const html = renderToStaticMarkup(
      <PermissionOperationView
        request={makeRequest({
          operation: { argv: [], cwd: FIXTURE_CWD, paths: [], networkDomains: ["registry.npmjs.org:443"], environmentNames: [] },
        })}
      />
    );
    expect(html).toContain("Agent 命令访问网络");
    expect(html).toContain("registry.npmjs.org");
    expect(html).toContain("端口 443");
    expect(html).toContain("https");
    expect(html).toContain("模型");
  });

  it("环境变量只展示名称并注明不含值", () => {
    const html = renderToStaticMarkup(
      <PermissionOperationView
        request={makeRequest({ operation: { argv: [], cwd: FIXTURE_CWD, paths: [], networkDomains: [], environmentNames: ["HOME"] } })}
      />
    );
    expect(html).toContain("HOME");
    expect(html).toContain("不包含变量值");
  });
});

describe("PermissionRequestCard (06 §9 第 5/12 条)", () => {
  it("展示谁/做什么/为什么", () => {
    const html = renderToStaticMarkup(<PermissionRequestCard request={makeRequest({ summary: "运行测试 bun test", reason: "验证改动" })} />);
    expect(html).toContain("运行测试 bun test");
    expect(html).toContain("验证改动");
    expect(html).toContain("开发 Agent");
  });

  it("不可授权请求解释原因与安全替代，不含允许文案", () => {
    const html = renderToStaticMarkup(
      <PermissionRequestCard request={makeRequest({ actionType: "system_change", riskLevel: "forbidden", grantable: false })} />
    );
    expect(html).toContain("不可授权");
    expect(html).toContain("安全方案");
  });
});

describe("PermissionRuleForm (06 §9 第 13 条)", () => {
  it("逐行展示实际规则而非「以后允许」", () => {
    const html = renderToStaticMarkup(
      <PermissionRuleForm request={makeRequest({ operation: { argv: ["bun", "add", "zod"], cwd: FIXTURE_CWD, paths: [], networkDomains: ["registry.npmjs.org:443"], environmentNames: [] } })} />
    );
    expect(html).toContain("将为当前项目保存这条精确规则");
    expect(html).toContain("可执行文件");
    expect(html).toContain("registry.npmjs.org:443");
    expect(html).not.toContain("以后允许");
  });
});
