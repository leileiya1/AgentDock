import { describe, expect, it } from "bun:test";
import type { FileDiff, Review } from "@/generated/bindings";
import { classifyFile, highRiskConfirmation, summarizeRisk, unresolvedIssues } from "./risk";

const file = (path: string, over: Partial<FileDiff> = {}): FileDiff => ({
  path,
  oldPath: null,
  binary: false,
  flagged: false,
  insertions: 5,
  deletions: 1,
  patch: null,
  ...over,
});

const deleted = (path: string) => file(path, { insertions: 0, deletions: 40 });

describe("文件风险分类 (05 §6.9)", () => {
  it("控制面文件被独立标记", () => {
    expect(classifyFile(file("CLAUDE.md"))).toContain("control_plane");
    expect(classifyFile(file(".agentflow/project.toml"))).toContain("control_plane");
    // 后端 flagged 是权威判定。
    expect(classifyFile(file("docs/rules.md", { flagged: true }))).toContain("control_plane");
  });

  it("只有被整体删除的测试才算「删除测试」", () => {
    expect(classifyFile(deleted("src/auth/login.test.ts"))).toContain("test_removed");
    // 改动测试不是风险。
    expect(classifyFile(file("src/auth/login.test.ts"))).not.toContain("test_removed");
    // 删除普通源码也不是「删除测试」。
    expect(classifyFile(deleted("src/auth/login.ts"))).not.toContain("test_removed");
  });

  it("权限 / 配置与安全敏感文件分别标记", () => {
    expect(classifyFile(file(".github/workflows/ci.yml"))).toContain("permission_config");
    expect(classifyFile(file("package.json"))).toContain("permission_config");
    expect(classifyFile(file("src/auth/session.ts"))).toContain("security_sensitive");
    expect(classifyFile(file("keys/server.pem"))).toContain("security_sensitive");
  });

  it("一个文件可以同时命中多个类别", () => {
    const kinds = classifyFile(deleted("tests/auth/session.test.ts"));
    expect(kinds).toContain("test_removed");
    expect(kinds).toContain("security_sensitive");
  });

  it("普通源码文件没有风险标记", () => {
    expect(classifyFile(file("src/utils/date.ts"))).toEqual([]);
  });
});

describe("风险汇总 (05 §6.9)", () => {
  const summary = summarizeRisk([
    file("src/utils/date.ts"),
    file("CLAUDE.md", { flagged: true }),
    deleted("src/auth/login.test.ts"),
    file("src/auth/login.test.ts".replace(".test", ".new.test")),
    file(".github/workflows/ci.yml"),
  ]);

  it("统计各类风险并标出高风险", () => {
    expect(summary.counts.control_plane).toBe(1);
    expect(summary.counts.test_removed).toBe(1);
    expect(summary.counts.permission_config).toBe(1);
    expect(summary.hasHighRisk).toBe(true);
  });

  it("区分被删除的测试与新增/修改的测试", () => {
    expect(summary.removedTests).toEqual(["src/auth/login.test.ts"]);
    expect(summary.touchedTests).toEqual(["src/auth/login.new.test.ts"]);
  });

  it("无风险改动不会误报", () => {
    const clean = summarizeRisk([file("src/utils/date.ts"), file("README.md")]);
    expect(clean.hasHighRisk).toBe(false);
    expect(clean.byFile.size).toBe(0);
  });
});

describe("未解决问题与批准确认 (05 §6.9)", () => {
  const review = (issues: Review["issues"]): Review => ({
    id: "r1",
    revision: 1,
    commitSha: "abc",
    decision: "request_changes",
    summary: null,
    issues,
  });
  const issue = (id: string, severity: Review["issues"][number]["severity"], resolved = false) => ({
    id,
    severity,
    file: null,
    lineStart: null,
    lineEnd: null,
    title: `问题 ${id}`,
    description: null,
    suggestedAction: null,
    resolved,
    agreementCount: 1,
  });

  it("只统计未解决的 critical / high", () => {
    const result = unresolvedIssues(
      review([issue("a", "critical"), issue("b", "high"), issue("c", "high", true), issue("d", "low")])
    );

    expect(result.critical).toBe(1);
    expect(result.high).toBe(1);
    expect(result.total).toBe(3);
    expect(result.requiresExtraConfirmation).toBe(true);
  });

  it("没有审查结果时不阻挡批准", () => {
    const result = unresolvedIssues(null);
    expect(result.requiresExtraConfirmation).toBe(false);
    expect(result.total).toBe(0);
  });

  it("确认文案具体到风险和文件，而不是「确定吗」", () => {
    const lines = highRiskConfirmation(
      summarizeRisk([file("CLAUDE.md", { flagged: true }), deleted("src/auth/login.test.ts")]),
      unresolvedIssues(review([issue("a", "critical")]))
    );

    expect(lines.join("\n")).toContain("1 个严重问题仍未解决");
    expect(lines.join("\n")).toContain("CLAUDE.md");
    expect(lines.join("\n")).toContain("src/auth/login.test.ts");
  });
});
