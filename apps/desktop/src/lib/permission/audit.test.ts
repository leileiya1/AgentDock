import { describe, expect, it } from "bun:test";
import {
  auditFacets,
  auditLine,
  auditVerb,
  EMPTY_FILTER,
  filterRequests,
  isActiveFilter,
} from "./audit";
import { makeRequest } from "./fixtures";

const list = [
  makeRequest({ id: "a", providerId: "claude", actionType: "command_execute", status: "approved", matchedRuleId: "rule-1" }),
  makeRequest({ id: "b", providerId: "codex", actionType: "dependency_install", status: "denied", matchedRuleId: null }),
  makeRequest({ id: "c", providerId: "claude", actionType: "external_path", status: "pending", matchedRuleId: null }),
];

describe("审计筛选 (06 §9 第 20 条)", () => {
  it("按 Provider 筛选", () => {
    expect(filterRequests(list, { ...EMPTY_FILTER, provider: "claude" }).map((r) => r.id)).toEqual(["a", "c"]);
  });
  it("按动作类型筛选", () => {
    expect(filterRequests(list, { ...EMPTY_FILTER, actionType: "dependency_install" }).map((r) => r.id)).toEqual(["b"]);
  });
  it("按决定筛选", () => {
    expect(filterRequests(list, { ...EMPTY_FILTER, decision: "denied" }).map((r) => r.id)).toEqual(["b"]);
  });
  it("按规则命中筛选", () => {
    expect(filterRequests(list, { ...EMPTY_FILTER, rule: "matched" }).map((r) => r.id)).toEqual(["a"]);
    expect(filterRequests(list, { ...EMPTY_FILTER, rule: "none" }).map((r) => r.id)).toEqual(["b", "c"]);
  });
  it("多维度组合", () => {
    expect(filterRequests(list, { ...EMPTY_FILTER, provider: "claude", decision: "pending" }).map((r) => r.id)).toEqual(["c"]);
  });
  it("facets 只列出实际出现的项", () => {
    const facets = auditFacets(list);
    expect(facets.providers.sort()).toEqual(["claude", "codex"]);
    expect(facets.actionTypes).toContain("external_path");
  });
  it("isActiveFilter 识别是否有筛选", () => {
    expect(isActiveFilter(EMPTY_FILTER)).toBe(false);
    expect(isActiveFilter({ ...EMPTY_FILTER, provider: "claude" })).toBe(true);
  });
});

describe("审计短摘要 (06 §9 第 21 条)", () => {
  it("决定映射为简短动词", () => {
    expect(auditVerb(makeRequest({ status: "approved" }))).toBe("允许");
    expect(auditVerb(makeRequest({ status: "denied" }))).toBe("拒绝");
    expect(auditVerb(makeRequest({ status: "pending" }))).toBe("请求");
  });
  it("审计行含动词、类别与短标题", () => {
    const line = auditLine(makeRequest({ status: "approved", actionType: "command_execute", summary: "运行测试" }));
    expect(line).toContain("允许");
    expect(line).toContain("执行命令");
    expect(line).toContain("运行测试");
  });
});
