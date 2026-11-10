import { expect, test } from "bun:test";
import { handleStudio, summariesWithMembers } from "./studio";

const call = (cmd: string, args: unknown = {}) => handleStudio(cmd, args, ["p1"], false)?.value as any;

test("预览 shim：外观 patch 不传不改、传 null 清空；成员影响项目汇总", () => {
  call("project_appearance_update", { projectId: "p1", patch: { kind: "team", roof: "#AABBCC" } });
  const next = call("project_appearance_update", { projectId: "p1", patch: { roof: null } });
  expect([next.kind, next.roof]).toEqual(["team", null]);
  expect(() => call("project_appearance_update", { projectId: "p1", patch: { roof: "red" } })).toThrow();

  call("project_members_set", { projectId: "p1", members: [{ agent: "codex", role: "both" }] });
  const [row] = summariesWithMembers([{ projectId: "p1", taskCount: 0, running: 0, waitingForYou: 0, done: 0, failing: 0, agents: [], activeAgents: [], lastActivityAt: null, assignedAgents: [], agentsSource: "derived" }]);
  expect([row.agentsSource, row.assignedAgents]).toEqual(["assigned", ["codex"]]);
  expect(() => call("project_members_set", { projectId: "p1", members: [{ agent: "codex", role: "both" }, { agent: "codex", role: "reviewer" }] })).toThrow();
  expect(handleStudio("task_list", {}, ["p1"], false)).toBeUndefined();
});
