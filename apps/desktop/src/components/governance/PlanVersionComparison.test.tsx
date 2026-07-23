import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { PlanReviewContext } from "@/generated/bindings";
import { PlanVersionComparison } from "./PlanVersionComparison";

const plan = (version: number, status: "rejected" | "pending", summary: string) => ({
  id: `p${version}`, version, status, summary, steps: [], risks: [], allowedPaths: [], planSha256: null,
  createdAt: "now", approvedAt: null,
});

describe("PlanVersionComparison", () => {
  it("shows rejection, scope changes and actual out-of-plan files", () => {
    const context: PlanReviewContext = {
      taskId: "t1",
      plans: [
        { plan: plan(1, "rejected", "old"), rejectionReason: "tests missing" },
        { plan: plan(2, "pending", "new"), rejectionReason: null },
      ],
      latestDiff: { fromVersion: 1, toVersion: 2, summaryChanged: true, addedSteps: ["test"], removedSteps: [], addedAllowedPaths: ["tests/**"], removedAllowedPaths: [], addedRisks: [], removedRisks: [] },
      detectedDeviations: ["escape.txt"], deviationPlanId: "p1", deviationDetectedAt: "now",
    };
    const html = renderToStaticMarkup(<PlanVersionComparison context={context} />);
    expect(html).toContain("计划 v1");
    expect(html).toContain("上次驳回：tests missing");
    expect(html).toContain("tests/**");
    expect(html).toContain("escape.txt");
    expect(html).toContain("上轮工作区已重置");
  });
});
