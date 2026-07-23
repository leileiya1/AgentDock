import type { AgentKind, RunRole, RunSummary } from "@/generated/bindings";

/** How an abnormal run ended — drives the §16 headline. */
export type RunFailureKind = "exit" | "timeout" | "interrupted";

export interface RunFailureSummary {
  role: RunRole;
  agent: AgentKind | null;
  kind: RunFailureKind;
  /** Process exit code when the run FAILED; null for timeouts/interruptions. */
  exitCode: number | null;
}

const ABNORMAL_STATUS: Partial<Record<RunSummary["status"], RunFailureKind>> = {
  FAILED: "exit",
  TIMED_OUT: "timeout",
  INTERRUPTED: "interrupted",
};

/**
 * Identify the run that most likely caused a `run_failed` / `agent_unresponsive` block: the last
 * abnormally-ended run of the current revision. `runList` returns runs ordered by created_at, so
 * iterating and keeping the last match yields the most recent failure. Returns null when the block
 * has no matching run (e.g. a validation-infra failure), letting callers fall back to generic copy.
 */
export function summarizeRunFailure(
  runs: RunSummary[],
  revision: number
): RunFailureSummary | null {
  let latest: RunFailureSummary | null = null;
  for (const run of runs) {
    if (run.revision !== revision) continue;
    const kind = ABNORMAL_STATUS[run.status];
    if (!kind) continue;
    latest = { role: run.role, agent: run.agent, kind, exitCode: run.exitCode };
  }
  return latest;
}
