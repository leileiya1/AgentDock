import { describe, expect, test } from "bun:test";
import { shouldDeferDialogEscape } from "./Dialog";

describe("Dialog Escape handling", () => {
  test("defers Escape while a portaled Select is open", () => {
    let queried = "";
    const open = shouldDeferDialogEscape({
      querySelector: (selector) => {
        queried = selector;
        return {};
      },
    });

    expect(open).toBe(true);
    expect(queried).toContain('select-content');
  });

  test("allows the Dialog to handle Escape without an open child", () => {
    expect(shouldDeferDialogEscape({ querySelector: () => null })).toBe(false);
  });

  test("defers when macOS already closed the popup and restored focus to its trigger", () => {
    expect(shouldDeferDialogEscape({
      querySelector: () => null,
      activeElement: { closest: (selector) => selector.includes("select-trigger") ? {} : null },
    })).toBe(true);
  });
});
