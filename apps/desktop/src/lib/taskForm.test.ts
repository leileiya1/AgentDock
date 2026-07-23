import { describe, expect, it } from "bun:test";
import { maxRevisionsError, parseMaxRevisions } from "./taskForm";

describe("最大返工轮数输入（P2-01）", () => {
  it("保留非法草稿并给出范围，而不是静默改成另一个数字", () => {
    expect(parseMaxRevisions("0")).toBeNull();
    expect(parseMaxRevisions("-1")).toBeNull();
    expect(maxRevisionsError("0")).toContain("1–20");
    expect(maxRevisionsError("-1")).toContain("整数");
  });

  it("只接受完整的 1 到 20 整数", () => {
    expect(parseMaxRevisions("1")).toBe(1);
    expect(parseMaxRevisions("20")).toBe(20);
    expect(parseMaxRevisions("21")).toBeNull();
    expect(parseMaxRevisions("1.5")).toBeNull();
    expect(parseMaxRevisions("")).toBeNull();
  });
});
