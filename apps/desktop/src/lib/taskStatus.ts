import type { TaskStatus } from "@/generated/bindings";

/**
 * These sets describe persisted task state, not visual preference. Keeping them in
 * one place prevents the list, detail view and execution tree from disagreeing
 * about whether work is still running.
 */
const EXECUTING = new Set<TaskStatus>([
  "PLANNING",
  "DEVELOPING",
  "VALIDATING",
  "REVIEWING",
  "REVISING",
  "MERGING",
]);

const AGENT_RUNNING = new Set<TaskStatus>([
  "PLANNING",
  "DEVELOPING",
  "REVIEWING",
  "REVISING",
]);

const TERMINAL = new Set<TaskStatus>(["MERGED", "ROLLED_BACK", "CANCELLED"]);

const QUEUED = new Set<TaskStatus>([
  "READY_FOR_DEVELOPMENT",
  "READY_FOR_REVIEW",
  "READY_FOR_REVISION",
]);

export function isTaskExecuting(status: TaskStatus): boolean {
  return EXECUTING.has(status);
}

export function isAgentRunning(status: TaskStatus): boolean {
  return AGENT_RUNNING.has(status);
}

export function isTaskTerminal(status: TaskStatus): boolean {
  return TERMINAL.has(status);
}

export function isTaskQueued(status: TaskStatus): boolean {
  return QUEUED.has(status);
}
