import { describe, expect, it } from "bun:test";
import type { QueueTaskState } from "@/generated/bindings";
import { canMutateQueue, queueSummary } from "./queue";

const base: QueueTaskState = {
  taskId: "task-1",
  state: "QUEUED",
  paused: false,
  priority: 0,
  position: 1,
  waitingReason: null,
  notBefore: null,
  lastError: null,
  attempts: 0,
  enqueuedAt: "2026-07-20T08:00:00Z",
  updatedAt: "2026-07-20T08:00:00Z",
};

describe("queue presentation", () => {
  it("explains persisted pause and queue position in user language", () => {
    expect(queueSummary({ ...base, paused: true, position: null, waitingReason: "paused" })).toBe("已暂停排队");
    expect(queueSummary({ ...base, position: 3, waitingReason: "tasks_ahead" })).toBe("队列第 3 位");
  });

  it("does not offer mutations after the scheduler has claimed the row", () => {
    expect(canMutateQueue(base)).toBe(true);
    expect(canMutateQueue({ ...base, state: "RUNNING" })).toBe(false);
    expect(canMutateQueue(null)).toBe(false);
  });
});
