import { describe, expect, it } from "bun:test";
import {
  canGrant,
  canMerge,
  expiryInfo,
  groupPendingRequests,
  hasExternalPath,
  hasPendingPermission,
  isProjectRuleRestricted,
  isStale,
  pathDisplay,
  pendingCount,
  scopeOptions,
} from "./model";
import { FIXTURE_CWD, makeRequest } from "./fixtures";

describe("权限授予与范围 (06 §9 第 11/12 条)", () => {
  it("不可授权动作没有任何范围且不能授予", () => {
    const req = makeRequest({ actionType: "system_change", riskLevel: "forbidden", grantable: false });
    expect(canGrant(req)).toBe(false);
    expect(scopeOptions(req)).toEqual([]);
  });

  it("grantable=false 的请求即使风险不高也不提供按钮", () => {
    const req = makeRequest({ grantable: false });
    expect(canGrant(req)).toBe(false);
    expect(scopeOptions(req)).toEqual([]);
  });

  it("高风险请求只提供一次/本任务，不提供项目规则", () => {
    const req = makeRequest({ riskLevel: "high" });
    expect(scopeOptions(req)).toEqual(["once", "task"]);
    expect(isProjectRuleRestricted(req)).toBe(true);
  });

  it("普通风险请求提供全部三种范围", () => {
    const req = makeRequest({ riskLevel: "medium" });
    expect(scopeOptions(req)).toEqual(["once", "task", "project_rule"]);
    expect(isProjectRuleRestricted(req)).toBe(false);
  });
});

describe("路径展示 (06 §9 第 7 条)", () => {
  it("工作树内路径压成相对路径", () => {
    const req = makeRequest();
    const view = pathDisplay(
      { path: `${FIXTURE_CWD}/src/app.ts`, access: "write", outsideWorktree: false },
      req.operation.cwd
    );
    expect(view.display).toBe("src/app.ts");
    expect(view.outsideWorktree).toBe(false);
  });

  it("工作树外路径保留完整位置并标记", () => {
    const req = makeRequest();
    const view = pathDisplay(
      { path: "/etc/hosts", access: "read", outsideWorktree: true },
      req.operation.cwd
    );
    expect(view.display).toBe("/etc/hosts");
    expect(view.outsideWorktree).toBe(true);
  });

  it("hasExternalPath 识别越界路径", () => {
    const req = makeRequest({
      operation: { argv: [], cwd: FIXTURE_CWD, paths: [{ path: "/etc/hosts", access: "read", outsideWorktree: true }], networkDomains: [], environmentNames: [] },
    });
    expect(hasExternalPath(req)).toBe(true);
  });
});

describe("请求合并 (06 §9 第 14 条)", () => {
  it("仅在操作摘要与权限完全相同时合并", () => {
    const a = makeRequest({ id: "a" });
    const b = makeRequest({ id: "b" });
    expect(canMerge(a, b)).toBe(true);
  });

  it("命令不同的请求不合并", () => {
    const a = makeRequest({ id: "a", operationSha256: "op-sha-aaa" });
    const b = makeRequest({ id: "b", operationSha256: "op-sha-bbb" });
    expect(canMerge(a, b)).toBe(false);
  });

  it("策略变化的相同命令不合并", () => {
    const a = makeRequest({ id: "a", policySha256: "policy-1" });
    const b = makeRequest({ id: "b", policySha256: "policy-2" });
    expect(canMerge(a, b)).toBe(false);
  });

  it("分组把相同操作折叠、不同操作分开", () => {
    const groups = groupPendingRequests([
      makeRequest({ id: "a", requestedAt: "2026-07-21T10:00:00Z" }),
      makeRequest({ id: "b", requestedAt: "2026-07-21T10:01:00Z" }),
      makeRequest({ id: "c", operationSha256: "op-sha-ccc", requestedAt: "2026-07-21T10:02:00Z" }),
    ]);
    expect(groups).toHaveLength(2);
    expect(groups[0].requests.map((r) => r.id)).toEqual(["a", "b"]);
    expect(groups[0].representative.id).toBe("b");
    expect(groups[1].requests.map((r) => r.id)).toEqual(["c"]);
  });

  it("分组只包含 pending 请求", () => {
    const groups = groupPendingRequests([
      makeRequest({ id: "a", status: "approved" }),
      makeRequest({ id: "b", status: "pending" }),
    ]);
    expect(groups).toHaveLength(1);
    expect(groups[0].representative.id).toBe("b");
  });
});

describe("弹窗作废 (06 §9 第 15 条)", () => {
  const shown = makeRequest();
  it("操作摘要变化即作废", () => {
    expect(isStale(shown, makeRequest({ operationSha256: "op-sha-bbb" }))).toBe(true);
  });
  it("策略变化即作废", () => {
    expect(isStale(shown, makeRequest({ policySha256: "policy-2" }))).toBe(true);
  });
  it("revision 变化即作废", () => {
    expect(isStale(shown, makeRequest({ revision: 3 }))).toBe(true);
  });
  it("请求已被处理即作废", () => {
    expect(isStale(shown, makeRequest({ status: "approved" }))).toBe(true);
  });
  it("请求消失即作废", () => {
    expect(isStale(shown, undefined)).toBe(true);
  });
  it("内容一致不作废", () => {
    expect(isStale(shown, makeRequest())).toBe(false);
  });
});

describe("有效期 (06 §9 第 16 条)", () => {
  it("剩余时间以分钟展示", () => {
    const req = makeRequest({ expiresAt: "2026-07-21T10:05:00Z" });
    const info = expiryInfo(req, Date.parse("2026-07-21T10:01:00Z"));
    expect(info.expired).toBe(false);
    expect(info.remainingLabel).toBe("4 分钟");
  });
  it("到期后标记 expired", () => {
    const req = makeRequest({ expiresAt: "2026-07-21T10:05:00Z" });
    const info = expiryInfo(req, Date.parse("2026-07-21T10:06:00Z"));
    expect(info.expired).toBe(true);
    expect(info.remainingLabel).toBeNull();
  });
  it("不足一分钟以秒展示", () => {
    const req = makeRequest({ expiresAt: "2026-07-21T10:05:00Z" });
    const info = expiryInfo(req, Date.parse("2026-07-21T10:04:30Z"));
    expect(info.remainingLabel).toBe("30 秒");
  });
});

describe("待处理请求指示 (06 §9 第 4 条)", () => {
  it("存在 pending 时标记等待", () => {
    expect(hasPendingPermission([makeRequest({ status: "pending" })])).toBe(true);
    expect(pendingCount([makeRequest({ status: "pending" }), makeRequest({ status: "approved" })])).toBe(1);
  });
  it("无 pending 时不标记", () => {
    expect(hasPendingPermission([makeRequest({ status: "approved" })])).toBe(false);
    expect(hasPendingPermission(undefined)).toBe(false);
  });
});
