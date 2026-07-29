import { describe, expect, it } from "bun:test";
import type { TaskEvent } from "@/generated/bindings";
import {
  latestValidationOutcome,
  latestValidationReport,
  parseTestReport,
  stepOutcome,
} from "./testReport";

const step = (over: Record<string, unknown>) => ({
  name: "test",
  argv: ["bun", "test"],
  exit_code: 0,
  duration_ms: 1200,
  stdout_tail: "",
  stderr_tail: "",
  ...over,
});

describe("parseTestReport", () => {
  it("parses the snake_case validation payload into typed steps", () => {
    const report = parseTestReport({
      schema_version: 1,
      passed: false,
      steps: [step({ name: "build", exit_code: 0 }), step({ name: "unit", exit_code: 1, stderr_tail: "boom" })],
    });
    expect(report?.passed).toBe(false);
    expect(report?.steps).toHaveLength(2);
    expect(report?.steps[1]).toMatchObject({ name: "unit", exitCode: 1, stderrTail: "boom" });
  });

  it("rejects payloads that are not a test report", () => {
    expect(parseTestReport(null)).toBeNull();
    expect(parseTestReport({ detail: "provider exited" })).toBeNull();
    expect(parseTestReport({ passed: true })).toBeNull();
  });
});

describe("stepOutcome", () => {
  it("distinguishes pass, timeout (null exit) and assertion failure", () => {
    expect(stepOutcome(parseTestReport({ passed: true, steps: [step({ exit_code: 0 })] })!.steps[0])).toBe("passed");
    expect(stepOutcome(parseTestReport({ passed: false, steps: [step({ exit_code: null })] })!.steps[0])).toBe("timeout");
    expect(stepOutcome(parseTestReport({ passed: false, steps: [step({ exit_code: 2 })] })!.steps[0])).toBe("failed");
  });
});

describe("latestValidationReport", () => {
  const ev = (over: Partial<TaskEvent>): TaskEvent => ({
    id: 1,
    taskId: "t",
    runId: null,
    revision: 2,
    actor: "orchestrator",
    eventType: "validation:failed",
    payload: { passed: false, steps: [step({ exit_code: 1 })] },
    createdAt: "2026-07-21T00:00:00Z",
    ...over,
  });

  it("returns the most recent validation report for the revision", () => {
    const events = [
      ev({ id: 1, eventType: "validation:passed", payload: { passed: true, steps: [step({ name: "old" })] } }),
      ev({ id: 2, eventType: "validation:failed", payload: { passed: false, steps: [step({ name: "new", exit_code: 1 })] } }),
    ];
    expect(latestValidationReport(events, 2)?.steps[0].name).toBe("new");
  });

  it("ignores other revisions, non-validation events, and empty (skipped) reports", () => {
    const events = [
      ev({ id: 1, revision: 1 }), // other revision
      ev({ id: 2, eventType: "run:succeeded", payload: { summary: "done" } }), // not validation
      ev({ id: 3, eventType: "validation:skipped", payload: { passed: true, steps: [] } }), // no steps
    ];
    expect(latestValidationReport(events, 2)).toBeNull();
  });

  it("classifies the validation outcome, distinguishing skipped from passed/failed (§9)", () => {
    const passed = [ev({ eventType: "validation:passed", payload: { passed: true, steps: [step({})] } })];
    expect(latestValidationOutcome(passed, 2)).toBe("passed");

    const failed = [ev({ eventType: "validation:failed", payload: { passed: false, steps: [step({ exit_code: 1 })] } })];
    expect(latestValidationOutcome(failed, 2)).toBe("failed");

    // Empty-steps report = no test/build command configured = the code was NOT validated.
    const skipped = [ev({ eventType: "validation:skipped", payload: { passed: true, steps: [] } })];
    expect(latestValidationOutcome(skipped, 2)).toBe("skipped");

    expect(latestValidationOutcome([], 2)).toBe("none");
  });
});
