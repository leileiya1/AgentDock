import { describe, expect, it } from "bun:test";
import type { ProviderDescriptor, TaskPreflightReport } from "@/generated/bindings";
import { pendingPreflightRoles, preflightBlockLine, preflightRoleLabel, summarizePreflight } from "@/lib/preflight";

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

describe("pendingPreflightRoles", () => {
  const provider = (id: string, displayName: string, planning = true): ProviderDescriptor => ({
    id,
    displayName,
    source: "builtin",
    protocolVersion: "1.0",
    capabilities: {
      planning,
      development: true,
      review: true,
      streaming: true,
      structuredOutput: true,
      sandbox: true,
      resume: true,
    },
    executionLocation: "local",
    dataEgress: "none",
    permissions: { worktreeRead: true, worktreeWrite: true, networkDomains: [], commands: [] },
    trust: "builtin",
    available: true,
    problem: null,
  });

  it("shows the actual parallel fallback chains instead of a fake percentage", () => {
    const roles = pendingPreflightRoles(
      {
        projectId: "p1",
        developerAgent: "qoder_cli",
        reviewerAgent: "grok_cli",
        allowApiEgress: false,
        requirePlanApproval: true,
      },
      {
        developerFallbacks: ["grok_cli", "deepseek_api"],
        reviewerFallbacks: ["qoder_cli"],
      },
      [provider("qoder_cli", "Qoder CLI"), provider("grok_cli", "Grok CLI")],
    );

    expect(roles[0].providers.map((item) => item.label)).toEqual(["Qoder CLI", "Grok CLI"]);
    expect(roles[1].providers.map((item) => item.label)).toEqual(["Grok CLI", "Qoder CLI"]);
    expect(roles.flatMap((role) => role.providers).some((item) => item.key === "deepseek_api")).toBe(false);
  });
});
