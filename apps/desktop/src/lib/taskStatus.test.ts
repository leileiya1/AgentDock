import { describe, expect, it } from "bun:test";
import { isAgentRunning, isTaskExecuting, isTaskQueued, isTaskTerminal } from "./taskStatus";

describe("persisted task status semantics", () => {
  it("distinguishes execution from queues and terminal states", () => {
    expect(isTaskExecuting("PLANNING")).toBe(true);
    expect(isTaskExecuting("MERGING")).toBe(true);
    expect(isTaskExecuting("READY_FOR_DEVELOPMENT")).toBe(false);
    expect(isTaskQueued("READY_FOR_DEVELOPMENT")).toBe(true);
    expect(isTaskTerminal("MERGED")).toBe(true);
    expect(isTaskTerminal("BLOCKED")).toBe(false);
  });

  it("only offers process cancellation while a supervised Agent exists", () => {
    expect(isAgentRunning("DEVELOPING")).toBe(true);
    expect(isAgentRunning("VALIDATING")).toBe(false);
    expect(isAgentRunning("MERGING")).toBe(false);
  });
});
