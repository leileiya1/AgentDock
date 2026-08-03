import { describe, expect, test } from "bun:test";
import { attentionNav } from "./attentionNav";

describe("attentionNav", () => {
  test("moves within the visible queue without wrapping", () => {
    expect(attentionNav("ArrowDown", 0, 3)).toEqual({ type: "focus", index: 1 });
    expect(attentionNav("ArrowDown", 2, 3)).toEqual({ type: "focus", index: 2 });
    expect(attentionNav("ArrowUp", 0, 3)).toEqual({ type: "focus", index: 0 });
  });

  test("supports first, last and open actions", () => {
    expect(attentionNav("Home", 2, 4)).toEqual({ type: "focus", index: 0 });
    expect(attentionNav("End", 0, 4)).toEqual({ type: "focus", index: 3 });
    expect(attentionNav("Enter", 2, 4)).toEqual({ type: "open", index: 2 });
    expect(attentionNav(" ", 1, 4)).toEqual({ type: "open", index: 1 });
  });

  test("leaves unrelated keys and empty queues alone", () => {
    expect(attentionNav("Tab", 0, 3)).toBeNull();
    expect(attentionNav("ArrowDown", 0, 0)).toBeNull();
  });
});
