import { describe, expect, it } from "bun:test";
import { BLOCKED_COPY } from "@/copy/blocked";

describe("failure recovery actions", () => {
  it("offers environment repair, detection, available-provider retry and fallback editing", () => {
    expect(BLOCKED_COPY.auth_expired.actions).toEqual([
      "providerSetup", "redetect", "retryAvailable", "editFallback", "cancel",
    ]);
    expect(BLOCKED_COPY.run_failed.actions).toContain("retryAvailable");
    expect(BLOCKED_COPY.run_failed.actions).toContain("editFallback");
  });
});
