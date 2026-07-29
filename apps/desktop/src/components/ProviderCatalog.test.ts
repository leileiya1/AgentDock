import { describe, expect, it } from "bun:test";
import type { ToolStatus } from "@/generated/bindings";
import { cliStatusText } from "./ProviderCatalog";

function status(partial: Partial<ToolStatus>): ToolStatus {
  return {
    found: true,
    path: "/usr/local/bin/provider",
    version: "1.0.0",
    compatible: true,
    problem: null,
    authenticated: null,
    authMethod: null,
    authProblem: null,
    supportLevel: "verified",
    verifiedVersions: ["1.0.0"],
    ...partial,
  };
}

describe("CLI Provider status copy", () => {
  it("never claims an auth-unknown CLI is connected", () => {
    const text = cliStatusText(status({ authenticated: null }));
    expect(text).toContain("已安装");
    expect(text).toContain("任务前实测");
    expect(text).not.toContain("已连接");
    expect(text).not.toContain("已登录");
  });

  it("distinguishes a confirmed login from an explicit login failure", () => {
    expect(cliStatusText(status({ authenticated: true }))).toContain("已登录");
    expect(cliStatusText(status({ authenticated: false, authProblem: "not signed in" }))).toBe("需要登录");
  });
});
