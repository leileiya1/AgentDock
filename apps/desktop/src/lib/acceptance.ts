import type { AcceptanceCriterionKind, ReviewDecision } from "@/generated/bindings";
import type { ValidationOutcome } from "@/lib/execution/testReport";

export type AcceptanceStatus = "passed" | "failed" | "pending" | "unverified" | "manual";

export function acceptanceStatus(
  kind: AcceptanceCriterionKind,
  validation: ValidationOutcome,
  review: ReviewDecision | null,
  manualConfirmed = false
): AcceptanceStatus {
  if (kind === "manual") return manualConfirmed ? "passed" : "manual";
  if (kind === "build" || kind === "test") {
    if (validation === "passed") return "passed";
    if (validation === "failed") return "failed";
    if (validation === "skipped") return "unverified";
    return "pending";
  }
  if (review === "pass") return "passed";
  if (review === "request_changes" || review === "block") return "failed";
  return "pending";
}
