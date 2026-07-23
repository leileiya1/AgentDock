import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { DeliveryRecord } from "@/generated/bindings";
import { CiChecksPanel } from "./CiChecksPanel";

const delivery: DeliveryRecord = {
  mode: "github_pr",
  state: "failed",
  remoteUrl: "https://github.test/pull/7",
  number: 7,
  ciStatus: "failed",
  ciChecks: [
    {
      name: "typecheck",
      status: "failed",
      required: true,
      workflow: "CI",
      description: "TypeScript validation",
      failureSummary: "src/main.ts: type mismatch",
      detailsUrl: "https://github.test/actions/runs/12/job/34",
      startedAt: "2026-07-20T10:00:00Z",
      completedAt: "2026-07-20T10:01:05Z",
    },
    {
      name: "lint",
      status: "passed",
      required: false,
      workflow: "CI",
      description: null,
      failureSummary: null,
      detailsUrl: null,
      startedAt: null,
      completedAt: null,
    },
  ],
  mergeCommit: null,
  preMergeCommit: null,
  rollbackCommit: null,
  updatedAt: "2026-07-20T10:02:00Z",
};

describe("CiChecksPanel", () => {
  it("renders check hierarchy, failure summary and a directly usable log link", () => {
    const html = renderToStaticMarkup(
      <CiChecksPanel delivery={delivery} refreshing={false} onRefresh={() => {}} />
    );
    expect(html).toContain("1/2 通过 · 1 失败");
    expect(html).toContain("typecheck");
    expect(html).toContain("必需");
    expect(html).toContain("src/main.ts: type mismatch");
    expect(html).toContain("远端日志");
    expect(html).toContain("https://github.test/actions/runs/12/job/34");
    expect(html).toContain("lint");
    expect(html).toContain("可选");
  });
});
