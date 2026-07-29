import { describe, expect, it } from "bun:test";
import { useUiStore } from "@/stores/uiStore";

describe("uiStore technical log disclosure", () => {
  it("keeps raw technical logs collapsed for a new session (P1-04)", () => {
    expect(useUiStore.getState().technicalLogOpen).toBe(false);
  });
});
