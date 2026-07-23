import { describe, expect, it } from "bun:test";
import { MASS_DELETE_THRESHOLD, isMassDeletion } from "./massDeletion";

describe("isMassDeletion", () => {
  it("warns at or above the threshold, stays quiet below it (§45)", () => {
    expect(isMassDeletion(MASS_DELETE_THRESHOLD)).toBe(true);
    expect(isMassDeletion(MASS_DELETE_THRESHOLD + 5)).toBe(true);
    expect(isMassDeletion(MASS_DELETE_THRESHOLD - 1)).toBe(false);
    expect(isMassDeletion(0)).toBe(false);
  });

  it("treats a missing count as no deletion", () => {
    expect(isMassDeletion(null)).toBe(false);
    expect(isMassDeletion(undefined)).toBe(false);
  });
});
