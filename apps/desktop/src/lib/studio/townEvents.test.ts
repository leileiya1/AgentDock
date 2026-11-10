import { describe, expect, test } from "bun:test";
import type { ProjectSummary } from "@/generated/bindings";
import { constructionCrew, hammerBeat, justBuilt, weedsFor } from "@/lib/studio/townEvents";
import { projectLook } from "@/lib/studio/townResidents";

const s = (over: Partial<ProjectSummary>) =>
  ({ projectId: "p", taskCount: 3, running: 0, waitingForYou: 0, done: 3, failing: 0, agents: [], activeAgents: [], lastActivityAt: null, assignedAgents: [], agentsSource: "derived", ...over }) as ProjectSummary;

describe("town events", () => {
  test("crew is the assigned agents, else two defaults, at most three", () => {
    expect(constructionCrew(undefined)).toEqual(["claude_code", "codex"]);
    expect(constructionCrew(s({ assignedAgents: ["qoder_cli"], agentsSource: "assigned" }))).toEqual(["qoder_cli"]);
    expect(constructionCrew(s({ agents: ["codex", "claude_code", "qoder_cli", "gemini_cli"] })).length).toBe(3);
  });

  test("weeds only after a month of quiet, with idle days counted", () => {
    const now = Date.parse("2026-07-30T00:00:00Z");
    const tenDays = projectLook(s({ lastActivityAt: "2026-07-20T00:00:00Z" }), now);
    expect(tenDays).toMatchObject({ leaves: true, weeds: false, idleDays: 10 });
    expect(projectLook(s({ lastActivityAt: "2026-06-01T00:00:00Z" }), now).weeds).toBe(true);
    expect(projectLook(s({ lastActivityAt: "2026-06-01T00:00:00Z", running: 1 }), now).weeds).toBe(false);
  });

  test("weeds are stable per project and stay in the yard", () => {
    expect(weedsFor("acme")).toEqual(weedsFor("acme"));
    expect(weedsFor("acme")).not.toEqual(weedsFor("docs"));
    for (const w of weedsFor("acme", 50)) {
      expect(w.x).toBeGreaterThanOrEqual(18);
      expect(w.x).toBeLessThanOrEqual(80);
      expect(w.blades.every((b) => b >= 4 && b <= 8)).toBe(true);
    }
  });

  test("hammer beat hops then sparks; still when reduced", () => {
    expect(hammerBeat(0.05, 0, false)).toEqual({ hop: 2, spark: false });
    expect(hammerBeat(0.15, 0, false)).toEqual({ hop: 0, spark: true });
    expect(hammerBeat(0.05, 0, true)).toEqual({ hop: 0, spark: false });
  });

  test("justBuilt fires once a site stops being a site", () => {
    expect(justBuilt(null, new Set(), ["a"])).toEqual([]);
    expect(justBuilt(new Set(["a", "b"]), new Set(["b"]), ["a", "b"])).toEqual(["a"]);
  });
});
