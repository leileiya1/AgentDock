import { expect, test } from "bun:test";
import type { TaskDetail } from "@/generated/bindings";
import { acceptanceCounts, resultHeadline } from "./taskResult";

const base = {
  status: "WAITING_FOR_HUMAN_APPROVAL",
  targetBranch: "main",
  acceptanceCriteria: [
    { id: "build", kind: "build", text: "可以构建" },
    { id: "behavior", kind: "behavior", text: "行为正确" },
    { id: "manual", kind: "manual", text: "人工确认" },
  ],
} as TaskDetail;

test("result headline states that final approval is still pending", () => {
  expect(resultHeadline(base).title).toBe("自动步骤已结束，等你验收");
});

test("acceptance totals keep manual confirmation distinct", () => {
  expect(acceptanceCounts(base, "passed", "pass")).toEqual({
    passed: 2,
    failed: 0,
    pending: 0,
    unverified: 0,
    manual: 1,
    total: 3,
  });
});
