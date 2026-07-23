import { describe, expect, it } from "bun:test";
import type { TaskPreflightReport } from "@/generated/bindings";
import { preflightBlockLine, preflightRoleLabel, summarizePreflight } from "@/lib/preflight";

function report(): TaskPreflightReport {
  return {
    ready: false,
    roles: [
      {
        role: "developer",
        primary: "claude_code",
        ready: false,
        chain: [
          {
            provider: "claude_code",
            displayName: "Claude Code",
            available: false,
            problem: "Claude Code 已安装，但尚未登录",
          },
          {
            provider: "gemini_cli",
            displayName: "Gemini CLI",
            available: false,
            problem: "gemini not found",
          },
        ],
      },
      {
        role: "reviewer",
        primary: "codex",
        ready: true,
        chain: [
          { provider: "codex", displayName: "Codex", available: true, problem: null },
        ],
      },
    ],
  };
}

describe("summarizePreflight", () => {
  it("keeps the full chain with per-provider reasons and isolates blocking roles", () => {
    const view = summarizePreflight(report());
    expect(view.ready).toBe(false);
    expect(view.roles).toHaveLength(2);

    const dev = view.roles[0];
    expect(dev.label).toBe("开发");
    expect(dev.ready).toBe(false);
    expect(dev.primaryLabel).toBe("Claude Code");
    expect(dev.providers.map((p) => p.label)).toEqual(["Claude Code", "Gemini CLI"]);
    expect(dev.providers[0].available).toBe(false);
    expect(dev.providers[0].problem).toContain("尚未登录");

    const reviewer = view.roles[1];
    expect(reviewer.ready).toBe(true);
    expect(reviewer.providers[0].available).toBe(true);

    // Only the unsatisfiable role is surfaced as blocking.
    expect(view.blockingRoles.map((r) => r.role)).toEqual(["developer"]);
  });

  it("reports ready with no blocking roles when every role can run", () => {
    const ok = report();
    ok.ready = true;
    ok.roles[0].ready = true;
    const view = summarizePreflight(ok);
    expect(view.ready).toBe(true);
    expect(view.blockingRoles).toHaveLength(0);
    expect(preflightBlockLine(view)).toContain("环境就绪");
  });
});

describe("preflightBlockLine", () => {
  it("names the blocking roles when a run cannot start", () => {
    const line = preflightBlockLine(summarizePreflight(report()));
    expect(line).toContain("开发");
    expect(line).not.toContain("审查");
  });
});

describe("preflightRoleLabel", () => {
  it("maps roles to human labels", () => {
    expect(preflightRoleLabel("developer")).toBe("开发");
    expect(preflightRoleLabel("reviewer")).toBe("审查");
  });
});
