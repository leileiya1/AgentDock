import { describe, expect, it } from "bun:test";
import type { RollbackPreflight } from "@/generated/bindings";
import { rollbackAllowed, rollbackBlockers, rollbackRecommendation } from "./rollback";

const preview: RollbackPreflight = {
  taskId: "task-1",
  deliveryMode: "local_merge",
  targetBranch: "main",
  currentBranch: "main",
  headCommit: "later",
  mergeCommit: "merge",
  preMergeCommit: "before",
  workingTreeClean: true,
  laterCommitCount: 1,
  laterCommits: [{ sha: "later", subject: "keep this work" }],
  affectedFileCount: 2,
  affectedFiles: ["src/a.ts", "src/b.ts"],
  affectedFilesTruncated: false,
  canUndo: false,
  undoBlockers: ["later_commits_exist"],
  canRevert: true,
  revertBlockers: [],
  recommendedStrategy: "revert",
  recommendationReason: "revert_preserves_later_commits",
  generatedAt: "2026-07-20T10:00:00Z",
};

describe("rollback preflight copy", () => {
  it("keeps undo blocked while recommending the history-preserving strategy", () => {
    expect(rollbackAllowed(preview, "undo")).toBe(false);
    expect(rollbackAllowed(preview, "revert")).toBe(true);
    expect(rollbackBlockers(preview, "undo")).toEqual(["merge commit 之后已有新提交"]);
    expect(rollbackRecommendation(preview)).toContain("保留后来工作");
  });
});
