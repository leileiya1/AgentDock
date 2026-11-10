import { expect, test } from "bun:test";
import type { TaskSummary } from "@/generated/bindings";
import { summarizeTasks } from "./projectSummary";

const t = (id: string, status: TaskSummary["status"], dev: string, rev: string, at: string, projectId = "p"): TaskSummary => ({
  id, projectId, seq: 1, title: id, status, blockedReason: null, currentRevision: 1, developerAgent: dev, reviewerAgent: rev, updatedAt: at,
});

test("分组规则与首页标签一致，与后端同一套", () => {
  const s = summarizeTasks("p", [
    t("a", "DEVELOPING", "claude_code", "codex", "2026-11-06T10:00:00+00:00"),
    t("b", "READY_FOR_REVIEW", "claude_code", "codex", "2026-11-06T09:00:00+00:00"),
    t("c", "WAITING_FOR_HUMAN_APPROVAL", "qoder_cli", "codex", "2026-11-06T08:00:00+00:00"),
    t("d", "APPROVED", "qoder_cli", "codex", "2026-11-06T07:00:00+00:00"),
    t("e", "MERGED", "gemini_cli", "codex", "2026-11-06T06:00:00+00:00"),
    t("x", "DEVELOPING", "kimi_api", "codex", "2026-11-06T11:00:00+00:00", "other"),
  ], "2026-10-07T00:00:00+00:00");
  expect([s.taskCount, s.running, s.waitingForYou, s.done]).toEqual([5, 2, 2, 1]);
  expect(s.activeAgents).toEqual(["claude_code"]);
  expect(s.lastActivityAt).toBe("2026-11-06T10:00:00+00:00");
});

test("只算近期参与的 Agent，失败只算未完结任务", () => {
  const s = summarizeTasks("p", [
    t("old", "MERGED", "grok_cli", "grok_api", "2026-09-01T00:00:00+00:00"),
    t("f", "READY_FOR_REVISION", "claude_code", "codex", "2026-11-06T10:00:00+00:00"),
    t("m", "MERGED", "claude_code", "codex", "2026-11-06T09:00:00+00:00"),
  ], "2026-10-07T00:00:00+00:00", new Set(["f", "m"]));
  expect(s.agents).toEqual(["claude_code", "codex"]);
  expect(s.failing).toBe(1);
});
