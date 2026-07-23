import type { TaskEvent } from "@/generated/bindings";

/** Mirrors the orchestrator's TestStepReport (serialized snake_case in the validation event). */
export interface TestStepReport {
  name: string;
  argv: string[];
  exitCode: number | null;
  durationMs: number;
  stdoutTail: string;
  stderrTail: string;
}

export interface TestReport {
  passed: boolean;
  steps: TestStepReport[];
}

/** How a single validation step ended — §18/§19 distinguish a timeout from an assertion failure. */
export type StepOutcome = "passed" | "timeout" | "failed";

export function stepOutcome(step: TestStepReport): StepOutcome {
  if (step.exitCode === 0) return "passed";
  // The orchestrator records a killed-by-timeout step with a null exit code.
  if (step.exitCode === null) return "timeout";
  return "failed";
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

/** Runtime-validate an untyped event payload into a TestReport, or null if it isn't one. */
export function parseTestReport(payload: unknown): TestReport | null {
  const root = asRecord(payload);
  if (!root || typeof root.passed !== "boolean" || !Array.isArray(root.steps)) return null;
  const steps: TestStepReport[] = [];
  for (const raw of root.steps) {
    const s = asRecord(raw);
    if (!s || typeof s.name !== "string") return null;
    steps.push({
      name: s.name,
      argv: Array.isArray(s.argv) ? s.argv.map(String) : [],
      exitCode: typeof s.exit_code === "number" ? s.exit_code : null,
      durationMs: typeof s.duration_ms === "number" ? s.duration_ms : 0,
      stdoutTail: typeof s.stdout_tail === "string" ? s.stdout_tail : "",
      stderrTail: typeof s.stderr_tail === "string" ? s.stderr_tail : "",
    });
  }
  return { passed: root.passed, steps };
}

/**
 * Find the most recent validation report for a revision, parsed from the validation event payload.
 * Returns null when the revision hasn't been validated yet (or was skipped with no steps).
 */
export function latestValidationReport(
  events: TaskEvent[],
  revision: number
): TestReport | null {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    const ev = events[i];
    if (ev.revision !== revision) continue;
    if (!ev.eventType.startsWith("validation:")) continue;
    const report = parseTestReport(ev.payload);
    if (report && report.steps.length > 0) return report;
  }
  return null;
}

/** Overall outcome of a revision's validation, for surfacing the §9/§18-19 "was it validated?" state. */
export type ValidationOutcome = "passed" | "failed" | "skipped" | "none";

/**
 * Classify a revision's validation. "skipped" means validation ran with zero configured steps —
 * i.e. the project has no test/build command, so the code was NOT validated. Surfacing this stops
 * unvalidated code from reaching approval silently (a project with genuinely no tests still passes,
 * but the human is told rather than misled).
 */
export function latestValidationOutcome(
  events: TaskEvent[],
  revision: number
): ValidationOutcome {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    const ev = events[i];
    if (ev.revision !== revision) continue;
    if (!ev.eventType.startsWith("validation:")) continue;
    const report = parseTestReport(ev.payload);
    if (!report) return "none";
    if (report.steps.length === 0) return "skipped";
    return report.passed ? "passed" : "failed";
  }
  return "none";
}
