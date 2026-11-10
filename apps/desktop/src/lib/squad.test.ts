import { expect, test } from "bun:test";
import type { TaskSummary } from "@/generated/bindings";
import { actingMember, buildSquad, relayStage, taskAvatar } from "./squad";

function task(seq: number, status: TaskSummary["status"], dev = "qoder_cli", rev = "codex"): TaskSummary {
  return { id: `t${seq}`, projectId: "p", seq, title: `任务 ${seq}`, status, blockedReason: null, currentRevision: 1, developerAgent: dev, reviewerAgent: rev, updatedAt: "2026-11-06T00:00:00Z" };
}

test("the baton follows the backend status, never a guess", () => {
  expect(relayStage(task(1, "DEVELOPING"))).toBe(1);
  expect(relayStage(task(1, "REVIEWING"))).toBe(3);
  expect(relayStage(task(1, "WAITING_FOR_HUMAN_APPROVAL"))).toBe(4);
  expect(relayStage(task(1, "BLOCKED"))).toBeNull();
  expect(actingMember(task(1, "VALIDATING"))).toBe("runner");
  expect(actingMember(task(1, "REVIEWING"))).toBe("codex");
  expect(actingMember(task(1, "WAITING_FOR_HUMAN_APPROVAL"))).toBeNull();
});

test("waiting tasks raise a hand, approvals show a finished reviewer", () => {
  expect(taskAvatar(task(1, "BLOCKED"))).toEqual({ member: "qoder_cli", mode: "hand" });
  expect(taskAvatar(task(1, "WAITING_FOR_HUMAN_APPROVAL"))).toEqual({ member: "codex", mode: "done" });
  expect(taskAvatar(task(1, "MERGED"))).toEqual({ member: "you", mode: "done" });
});

test("only members with a running task animate in the squad", () => {
  const squad = buildSquad([task(13, "DEVELOPING"), task(9, "BLOCKED", "claude_code"), task(12, "WAITING_FOR_HUMAN_APPROVAL")]);
  const qoder = squad.slots.find((slot) => slot.member === "qoder_cli");
  const claude = squad.slots.find((slot) => slot.member === "claude_code");
  expect(qoder?.mode).toBe("work");
  expect(qoder?.working).toBe("TASK-013");
  expect(claude?.mode).toBe("idle");
  expect(squad.slots.at(-1)?.member).toBe("runner");
  expect(squad.waitingForYou).toBe(2);
});
