import { describe, expect, it } from "bun:test";
import {
  ruleIsEffective,
  rulePreview,
  ruleStatus,
  sortRules,
  summarizeRules,
} from "./rules";
import { makeRequest, makeRule } from "./fixtures";

describe("项目规则预览 (06 §9 第 13 条)", () => {
  it("展示可执行文件与参数模板，而不是一句「以后允许」", () => {
    const lines = rulePreview(makeRequest({ operation: { argv: ["bun", "test", "src"], cwd: "/w", paths: [], networkDomains: [], environmentNames: [] } }));
    const labels = lines.map((l) => l.label);
    expect(labels).toContain("可执行文件");
    expect(labels).toContain("参数模板");
    expect(lines.find((l) => l.label === "可执行文件")?.value).toBe("bun");
    expect(lines.find((l) => l.label === "参数模板")?.value).toBe("test src");
    expect(lines.some((l) => /以后允许/.test(l.value))).toBe(false);
  });

  it("网络与环境变量以精确值呈现", () => {
    const lines = rulePreview(
      makeRequest({
        actionType: "dependency_install",
        operation: { argv: ["bun", "install"], cwd: "/w", paths: [], networkDomains: ["registry.npmjs.org:443"], environmentNames: ["HOME"] },
      })
    );
    expect(lines.find((l) => l.label === "允许域名")?.value).toBe("registry.npmjs.org:443");
    expect(lines.find((l) => l.label === "允许环境变量")?.value).toBe("HOME");
  });

  it("工作树外路径单独标注", () => {
    const lines = rulePreview(
      makeRequest({
        actionType: "external_path",
        operation: { argv: [], cwd: "/w", paths: [{ path: "/etc/hosts", access: "read", outsideWorktree: true }], networkDomains: [], environmentNames: [] },
      })
    );
    expect(lines.find((l) => l.label === "工作树外路径")?.value).toContain("/etc/hosts");
  });
});

describe("规则状态 (06 §9 第 18/19 条)", () => {
  const now = Date.parse("2026-07-21T12:00:00Z");
  it("撤销优先于其它状态", () => {
    const rule = makeRule({ revokedAt: "2026-07-21T11:00:00Z", enabled: false });
    expect(ruleStatus(rule, now)).toBe("revoked");
    expect(ruleIsEffective(rule, now)).toBe(false);
  });
  it("停用规则不生效", () => {
    expect(ruleStatus(makeRule({ enabled: false }), now)).toBe("disabled");
  });
  it("到期规则不生效", () => {
    const rule = makeRule({ expiresAt: "2026-07-21T11:00:00Z" });
    expect(ruleStatus(rule, now)).toBe("expired");
    expect(ruleIsEffective(rule, now)).toBe(false);
  });
  it("正常规则生效中", () => {
    expect(ruleStatus(makeRule(), now)).toBe("active");
    expect(ruleIsEffective(makeRule(), now)).toBe(true);
  });
});

describe("规则排序与汇总", () => {
  const now = Date.parse("2026-07-21T12:00:00Z");
  it("生效规则排在撤销规则之前", () => {
    const sorted = sortRules([
      makeRule({ id: "revoked", revokedAt: "2026-07-21T11:00:00Z" }),
      makeRule({ id: "active", lastMatchedAt: "2026-07-21T11:30:00Z" }),
    ], now);
    expect(sorted[0].id).toBe("active");
  });
  it("汇总只计生效规则", () => {
    const summary = summarizeRules([
      makeRule({ id: "a" }),
      makeRule({ id: "b", enabled: false }),
    ], now);
    expect(summary.total).toBe(2);
    expect(summary.active).toBe(1);
  });
});
