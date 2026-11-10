import { expect, test } from "bun:test";
import type { ProjectSummary } from "@/generated/bindings";
import { arrangeProjects, greeting, onDuty, projectStatus } from "./sidebarModel";

const s = (over: Partial<ProjectSummary>): ProjectSummary => ({
  projectId: "p", taskCount: 4, running: 0, waitingForYou: 0, done: 4, failing: 0, agents: [], activeAgents: [],
  lastActivityAt: new Date().toISOString(), assignedAgents: [], agentsSource: "derived", ...over,
});

test("侧栏状态一句话：失败 > 等你 > 在跑 > 很久没动 > 已完成", () => {
  expect(projectStatus(undefined).tone).toBe("new");
  expect(projectStatus(s({ failing: 2, waitingForYou: 1 }))).toMatchObject({ tone: "danger", waiting: 1 });
  expect(projectStatus(s({ waitingForYou: 3, running: 1 }))).toMatchObject({ tone: "human", line: "3 件等你拍板", running: true });
  expect(projectStatus(s({ running: 2 })).line).toBe("2 件在跑");
  const old = new Date(Date.now() - 5 * 24 * 3600 * 1000).toISOString();
  expect(projectStatus(s({ lastActivityAt: old })).line).toBe("5 天没动了");
  expect(projectStatus(s({})).line).toBe("4 件已完成");
});

test("在岗的 Agent 跨项目去重，问候按时间", () => {
  expect(onDuty([s({ activeAgents: ["codex", "claude_code"] }), s({ activeAgents: ["codex"] })])).toEqual(["codex", "claude_code"]);
  expect(greeting(new Date(2026, 0, 1, 9)).night).toBe(false);
  expect(greeting(new Date(2026, 0, 1, 22)).text).toBe("晚上好，老板");
});

test("置顶在上按置顶顺序，其余按排序方式", () => {
  const r = (id: string, waiting: number, running: boolean, at: string | null) => ({ id, waiting, running, lastActivityAt: at });
  const rows = [r("a", 0, false, "2026-07-01"), r("b", 2, false, "2026-07-02"), r("c", 0, true, "2026-07-03"), r("d", 1, false, null)];
  const ids = (xs: { id: string }[]) => xs.map((x) => x.id);
  const def = arrangeProjects(rows, ["c", "gone", "a"], "default");
  expect(ids(def.pinned)).toEqual(["c", "a"]);
  expect(ids(def.rest)).toEqual(["b", "d"]);
  expect(ids(arrangeProjects(rows, [], "waiting").rest)).toEqual(["b", "d", "c", "a"]);
  expect(ids(arrangeProjects(rows, [], "recent").rest)).toEqual(["c", "b", "a", "d"]);
});
