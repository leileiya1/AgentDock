import { describe, expect, test } from "bun:test";
import { dialogChildClosedRecently, noteDialogChildClosed } from "./dialogChildOverlay";

describe("dialog child overlay timing", () => {
  test("defers the Escape emitted while macOS finishes closing a Select popup", () => {
    noteDialogChildClosed(10_000);
    expect(dialogChildClosedRecently(10_999)).toBe(true);
  });

  test("does not swallow a later intentional Dialog Escape", () => {
    noteDialogChildClosed(10_000);
    expect(dialogChildClosedRecently(11_001)).toBe(false);
  });
});
