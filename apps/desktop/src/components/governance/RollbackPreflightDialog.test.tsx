import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { RollbackPreflight } from "@/generated/bindings";
import { RollbackPreflightDialog } from "./RollbackPreflightDialog";

const preflight: RollbackPreflight = {
  taskId: "task-1", deliveryMode: "local_merge", targetBranch: "main", currentBranch: "main",
  headCommit: "later123", mergeCommit: "merge123", preMergeCommit: "base123",
  workingTreeClean: true, laterCommitCount: 1,
  laterCommits: [{ sha: "later123", subject: "keep user work" }],
  affectedFileCount: 2, affectedFiles: ["src/a.ts", "src/b.ts"], affectedFilesTruncated: false,
  canUndo: false, undoBlockers: ["later_commits_exist"], canRevert: true, revertBlockers: [],
  recommendedStrategy: "revert", recommendationReason: "revert_preserves_later_commits",
  generatedAt: "2026-07-20T10:00:00Z",
};

describe("RollbackPreflightDialog", () => {
  it("shows impact and disables an unsafe undo", () => {
    const html = renderToStaticMarkup(
      <RollbackPreflightDialog open strategy="undo" preflight={preflight} loading={false} error={false}
        confirming={false} onClose={() => {}} onRetry={() => {}} onConfirm={() => {}} />
    );
    expect(html).toContain("当前策略不可执行");
    expect(html).toContain("merge commit 之后已有新提交");
    expect(html).toContain("将影响 2 个文件");
    expect(html).toContain("src/a.ts");
    expect(html).toContain("keep user work");
    expect(html).toContain("disabled");
  });
});
