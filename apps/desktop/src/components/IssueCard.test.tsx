import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { ReviewIssue } from "@/generated/bindings";
import { IssueCard } from "./IssueCard";

function issue(over: Partial<ReviewIssue>): ReviewIssue {
  return {
    id: "i1",
    severity: "high",
    file: "src/auth.rs",
    lineStart: 12,
    lineEnd: 12,
    title: "缺少输入校验",
    description: null,
    suggestedAction: null,
    resolved: false,
    reportedBy: [],
    agreementCount: 1,
    severityDisagreement: false,
    ...over,
  };
}

describe("IssueCard quality markers (§24/§25)", () => {
  it("flags a severity disagreement when reviewers split", () => {
    const html = renderToStaticMarkup(<IssueCard issue={issue({ severityDisagreement: true })} />);
    expect(html).toContain("严重度存在分歧");
  });

  it("flags an issue that has no file location as needing manual verification", () => {
    const html = renderToStaticMarkup(<IssueCard issue={issue({ file: null, lineStart: null })} />);
    expect(html).toContain("无文件定位");
  });

  it("shows neither marker for a well-formed, agreed issue", () => {
    const html = renderToStaticMarkup(<IssueCard issue={issue({})} />);
    expect(html).not.toContain("严重度存在分歧");
    expect(html).not.toContain("无文件定位");
  });
});
