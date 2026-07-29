import { describe, expect, it } from "bun:test";
import type { RunSummary } from "@/generated/bindings";
import { summarizeRunFailure } from "./runFailure";

function run(partial: Partial<RunSummary>): RunSummary {
  return {
    id: "r",
    taskId: "t",
    revision: 2,
    role: "developer",
    agent: "claude_code",
    status: "SUCCEEDED",
    exitCode: 0,
    costUsd: null,
    tokensIn: null,
    tokensOut: null,
    startedAt: null,
    finishedAt: null,
    ...partial,
  };
}

describe("summarizeRunFailure", () => {
  it("returns the last abnormal run of the current revision", () => {
    const runs = [
      run({ id: "a", status: "FAILED", exitCode: 1, agent: "codex" }),
      run({ id: "b", status: "FAILED", exitCode: 2, agent: "claude_code" }),
    ];
    const failure = summarizeRunFailure(runs, 2);
    expect(failure).toEqual({ role: "developer", agent: "claude_code", kind: "exit", exitCode: 2 });
  });

  it("classifies timeouts and interruptions distinctly", () => {
    expect(summarizeRunFailure([run({ status: "TIMED_OUT", exitCode: null })], 2)?.kind).toBe("timeout");
    expect(summarizeRunFailure([run({ status: "INTERRUPTED", exitCode: null })], 2)?.kind).toBe("interrupted");
  });

  it("ignores other revisions and non-abnormal runs", () => {
    const runs = [
      run({ revision: 1, status: "FAILED", exitCode: 9 }), // old revision
      run({ revision: 2, status: "SUCCEEDED" }),
      run({ revision: 2, status: "RUNNING" }),
      run({ revision: 2, status: "CANCELLED" }),
    ];
    expect(summarizeRunFailure(runs, 2)).toBeNull();
  });

  it("returns null when there are no runs", () => {
    expect(summarizeRunFailure([], 2)).toBeNull();
  });
});
