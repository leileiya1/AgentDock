import { describe, expect, it } from "bun:test";
import type { EnvReport, OnboardingReport, ProviderStatus, ToolStatus } from "@/generated/bindings";
import { buildDots, summarizeEnvironment, toolDot } from "@/lib/envDots";

function tool(partial: Partial<ToolStatus>): ToolStatus {
  return {
    found: true,
    path: "/usr/local/bin/cli",
    version: "1.0.0",
    compatible: true,
    problem: null,
    authenticated: null,
    authMethod: null,
    authProblem: null,
    supportLevel: "verified",
    verifiedVersions: ["1.0.0"],
    ...partial,
  };
}

function api(): ProviderStatus {
  return { configured: false, available: false, model: "m", baseUrl: "https://x", keyEnv: "K", problem: null };
}

function envReport(claude: ToolStatus = tool({ authenticated: true })): EnvReport {
  return {
    system: {
      os: "macos", osVersion: "15.5", architecture: "aarch64", agentflowVersion: "0.1.0",
      shell: "/bin/zsh", diskAvailableBytes: 1024 ** 3,
      network: { available: true, detail: "ok", problem: null },
      keychain: { available: true, detail: "login", problem: null },
    },
    git: tool({}),
    node: tool({}),
    bun: tool({}),
    claudeCode: claude,
    codex: tool({ authenticated: true }),
    geminiCli: tool({ found: false, compatible: false }),
    qwenCode: tool({ found: false, compatible: false }),
    qoderCli: tool({ found: false, compatible: false }),
    grokCli: tool({}),
    kimiCli: tool({}),
    minimaxCli: tool({}),
    openaiApi: api(),
    anthropicApi: api(),
    deepseekApi: api(),
    grokApi: api(),
    minimaxApi: api(),
    kimiApi: api(),
  };
}

describe("toolDot", () => {
  it("is ready only when installed, compatible and not explicitly unauthenticated", () => {
    const dot = toolDot("claude", "Claude Code", tool({ authenticated: true }), false);
    expect(dot.tone).toBe("ok");
    expect(dot.blocking).toBe(false);
    expect(dot.label).toContain("就绪");
  });

  it("treats an installed-but-not-logged-in CLI as a blocker, not ready (P0-01)", () => {
    const dot = toolDot(
      "claude",
      "Claude Code",
      tool({ authenticated: false, authProblem: "Claude Code 已安装，但尚未登录" }),
      false,
    );
    expect(dot.tone).toBe("bad");
    expect(dot.blocking).toBe(true);
    expect(dot.label).toContain("尚未登录");
  });

  it("keeps auth-unknown CLIs (gemini/qwen) ready when installed and compatible", () => {
    // Gemini/Qwen expose no side-effect-free auth check, so authenticated stays null.
    const dot = toolDot("gemini", "Gemini CLI", tool({ authenticated: null }), true);
    expect(dot.tone).toBe("ok");
  });

  it("shows an optional missing CLI as installable, not blocking", () => {
    const dot = toolDot("qwen", "Qwen Code", tool({ found: false, compatible: false }), true);
    expect(dot.tone).toBe("idle");
    expect(dot.blocking).toBe(false);
  });
});

describe("buildDots", () => {
  it("marks the environment blocked when a required CLI is unauthenticated", () => {
    const env = envReport(tool({ authenticated: false, authProblem: "尚未登录" }));
    const dots = buildDots(env, true);
    const claude = dots.find((d) => d.key === "claude");
    expect(claude?.blocking).toBe(true);
    expect(dots.some((d) => d.blocking)).toBe(true);
  });
});

describe("summarizeEnvironment", () => {
  it("reports the end-to-end environment only when the workflow and required tools are ready", () => {
    const summary = summarizeEnvironment({
      workflowReady: true,
      daemonRunning: true,
      appReady: true,
      env: envReport(),
      notices: [],
    } as unknown as OnboardingReport);

    expect(summary.blocking).toBe(false);
    expect(summary.label).toBe("端到端环境就绪");
  });

  it("surfaces the first blocking fact instead of claiming readiness", () => {
    const summary = summarizeEnvironment({
      workflowReady: false,
      daemonRunning: true,
      appReady: true,
      env: envReport(tool({ authenticated: false, authProblem: "尚未登录" })),
      notices: [],
    } as unknown as OnboardingReport);

    expect(summary.blocking).toBe(true);
    expect(summary.label).toBe("应用可用 · 工作流未就绪");
    expect(summary.detail).toContain("尚未登录");
  });
});
