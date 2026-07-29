import { expect, test } from "bun:test";
import type { TaskDetail } from "@/generated/bindings";
import type { ExecutionTree } from "./tree";
import { buildAgentConsole } from "./agentConsole";

const task = {
  id: "t1",
  developerAgent: "qoder_cli",
  reviewerAgent: "grok_cli",
  currentRevision: 2,
  policy: { requirePlanApproval: false },
} as TaskDetail;

test("agent console exposes parallel members and fallback attempts", () => {
  const tree = {
    currentRevision: 2,
    systemEvents: [],
    revisions: [{
      revision: 2,
      state: "attention",
      startedAt: null,
      endedAt: null,
      conclusion: "当前轮",
      phases: [{
        phase: "review",
        label: "独立审查",
        state: "attention",
        summary: "委员会 2/2 已返回",
        events: [],
        startedAt: null,
        endedAt: null,
        groups: [
          {
            key: "one",
            role: "reviewer",
            memberIndex: 1,
            memberTotal: 2,
            state: "ok",
            attempts: [
              { runId: "r1", agent: "qoder_cli", status: "FAILED", state: "failed", attemptLabel: "首选", fallbackReason: null, startedAt: null, finishedAt: null, events: [], recovery: [] },
              { runId: "r2", agent: "grok_cli", status: "SUCCEEDED", state: "ok", attemptLabel: "降级 1", fallbackReason: "额度不足", startedAt: null, finishedAt: null, events: [], recovery: [] },
            ],
            current: { runId: "r2", agent: "grok_cli", status: "SUCCEEDED", state: "ok", attemptLabel: "降级 1", fallbackReason: "额度不足", startedAt: null, finishedAt: null, events: [], recovery: [] },
          },
          {
            key: "two",
            role: "reviewer",
            memberIndex: 2,
            memberTotal: 2,
            state: "attention",
            attempts: [{ runId: "r3", agent: "codex", status: "INTERRUPTED", state: "attention", attemptLabel: "首选", fallbackReason: null, startedAt: null, finishedAt: null, events: [], recovery: [] }],
            current: { runId: "r3", agent: "codex", status: "INTERRUPTED", state: "attention", attemptLabel: "首选", fallbackReason: null, startedAt: null, finishedAt: null, events: [], recovery: [] },
          },
        ],
      }],
    }],
  } satisfies ExecutionTree;

  const lanes = buildAgentConsole(task, tree);
  expect(lanes).toHaveLength(4);
  expect(lanes[0]).toMatchObject({ state: "info", agents: [], summary: "当前流程未启用" });
  expect(lanes[1].agents).toEqual(["qoder_cli"]);
  expect(lanes[2].agents).toEqual([]);
  expect(lanes[3]).toMatchObject({ memberCount: 2, fallbackCount: 1, runId: "r3" });
  expect(lanes[3].agents).toEqual(["grok_cli", "codex"]);
});

test("agent console carries a completed planning phase from its real earlier revision", () => {
  const plannedTask = { ...task, policy: { requirePlanApproval: true } } as TaskDetail;
  const tree = {
    currentRevision: 2,
    systemEvents: [],
    revisions: [
      { revision: 2, state: "ok", startedAt: null, endedAt: null, conclusion: "当前轮", phases: [] },
      {
        revision: 0,
        state: "ok",
        startedAt: null,
        endedAt: null,
        conclusion: "本轮已结束",
        phases: [{
          phase: "plan",
          label: "计划",
          state: "ok",
          summary: "你批准了计划，计划已锁定",
          groups: [],
          events: [],
          startedAt: null,
          endedAt: null,
        }],
      },
    ],
  } satisfies ExecutionTree;

  expect(buildAgentConsole(plannedTask, tree)[0]).toMatchObject({
    revision: 0,
    state: "ok",
    summary: "你批准了计划，计划已锁定",
  });
});
