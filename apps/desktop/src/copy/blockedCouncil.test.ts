import { describe, expect, test } from "bun:test";
import { BLOCKED_COPY, COUNCIL_REQUIRED_COPY, blockedCopyFor, latestGateEvent } from "@/copy/blocked";

const ev = (id: number, eventType: string) => ({ id, eventType });

describe("quality gate card picks the real cause", () => {
  test("council required before any review → council card with setup + re-review", () => {
    const gate = latestGateEvent([ev(10, "validation:passed"), ev(11, "integrity:security_review_required")]);
    const copy = blockedCopyFor("quality_gate", gate);
    expect(copy).toBe(COUNCIL_REQUIRED_COPY);
    expect(copy?.actions).toContain("councilSetup");
    expect(copy?.actions).toContain("reReview");
    expect(copy?.actions).not.toContain("guidance");
  });

  test("a later review outcome wins over an old council requirement", () => {
    const gate = latestGateEvent([ev(11, "integrity:security_review_required"), ev(20, "review:council_block"), ev(21, "quality:gate_failed")]);
    expect(blockedCopyFor("quality_gate", gate)).toBe(BLOCKED_COPY.quality_gate);
  });

  test("other reasons are untouched", () => {
    expect(blockedCopyFor("run_failed", "integrity:security_review_required")).toBe(BLOCKED_COPY.run_failed);
    expect(blockedCopyFor(null, null)).toBeNull();
  });
});
