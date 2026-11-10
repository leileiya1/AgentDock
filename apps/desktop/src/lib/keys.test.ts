import { expect, test } from "bun:test";
import { stepIndex } from "@/lib/keys";

test("stepIndex walks and clamps", () => {
  expect(stepIndex("j", -1, 3)).toBe(0);
  expect(stepIndex("k", -1, 3)).toBe(2);
  expect(stepIndex("j", 2, 3)).toBe(2);
  expect(stepIndex("K", 1, 3)).toBe(0);
  expect(stepIndex("x", 1, 3)).toBeNull();
  expect(stepIndex("j", 0, 0)).toBeNull();
});
