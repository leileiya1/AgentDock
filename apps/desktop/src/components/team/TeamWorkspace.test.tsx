import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { TeamView } from "@/generated/bindings";
import { TeamFacts } from "./TeamWorkspace";
import { DEFAULT_TEAM_CONFIG } from "./TeamTaskForm";
import { TeamRecoveryFacts } from "./TeamRecoveryPanel";
import { TeamPlanImpact } from "./TeamPlanEditor";
const view: TeamView = {
  taskId: "t", phase: "running", originalGoal: "real goal", config: DEFAULT_TEAM_CONFIG,
  requirements: [{ id: "r", originalText: "negative case", source: "user", required: true }],
  plan: null, planRevision: 1, items: [], attempts: [], artifacts: [], candidate: null,
  criteria: [{ criterion: { id: "negative", text: "wrong password", requirementIds: ["r"], required: true, expected: "explicit error", ownerWorkItemId: "dev", checks: [] }, status: "not_run", stale: false, evidence: [] }],
  gateBlockers: ["negative: not_run"], attemptsUsed: 0, reworksUsed: 0, detail: null, reportPath: null, limitations: [],
};
describe("team facts use backend criterion state", () => {
  test("unrun behavior is never inferred from overall progress", () => {
    const html = renderToStaticMarkup(<TeamFacts view={view} />);
    expect(html).toContain("未测"); expect(html).toContain("negative: not_run"); expect(html).toContain("尚未完成集成");
    expect(html).not.toContain("text-status-success"); expect(html).toContain("费用和 Token 为未知");
  });
  test("historical passing evidence stays stale on a new candidate", () => {
    const updated = { ...view, criteria: [{ ...view.criteria[0], status: "not_run" as const, stale: true }] };
    const html = renderToStaticMarkup(<TeamFacts view={updated} />);
    expect(html).toContain("证据已失效"); expect(html).not.toContain("text-status-success");
  });
});
describe("team recovery and plan impact facts", () => {
  test("an unknown process remains visibly occupied and never implies acceptance", () => {
    const html=renderToStaticMarkup(<TeamRecoveryFacts recovery={{taskId:"t",phase:"blocked",resources:[{attemptId:"old",workItemId:"a",memberId:"dev-a",provider:"codex",failureClass:"result_unknown",cancelRequested:true,state:"unknown",detail:"保留旧资源"}],attemptsRemaining:0,reworksRemaining:0,targetBase:"old",targetCurrent:"new",targetChanged:true,configChanged:false,notices:["额度不重置"],inspectedAt:"backend-time"}}/>);
    expect(html).toContain("进程结果未知");expect(html).toContain("保留占用");expect(html).toContain("backend-time");expect(html).toContain("剩余尝试 0");expect(html).not.toContain("text-status-success");
  });
  test("plan impact keeps preserved work separate from invalidated inputs", () => {
    const html=renderToStaticMarkup(<TeamPlanImpact preview={{baseRevision:3,proposalHash:"backend-hash",added:[],removed:[],changed:["a"],affected:["a","dependent"],preserved:["unrelated"],blockingAttemptIds:["running-a"],acceptanceChanged:false,blockers:["stop affected attempts first"]}}/>);
    expect(html).toContain("保留");expect(html).toContain("unrelated");expect(html).toContain("dependent");expect(html).toContain("running-a");expect(html).toContain("stop affected attempts first");expect(html).toContain("累计额度不重置");
  });
});
