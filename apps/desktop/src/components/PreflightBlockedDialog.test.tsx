import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { TaskPreflightReport } from "@/generated/bindings";
import { summarizePreflight } from "@/lib/preflight";
import { PreflightReportBody } from "./PreflightBlockedDialog";

const report: TaskPreflightReport = {
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
        { provider: "codex", displayName: "Codex", available: true, problem: null },
      ],
    },
    {
      role: "reviewer",
      primary: "codex",
      ready: true,
      chain: [{ provider: "codex", displayName: "Codex", available: true, problem: null }],
    },
  ],
};

describe("PreflightReportBody", () => {
  it("lists every provider in the chain with its reason and the role verdict", () => {
    const html = renderToStaticMarkup(<PreflightReportBody view={summarizePreflight(report)} />);
    // Both roles rendered with their primary.
    expect(html).toContain("开发角色");
    expect(html).toContain("审查角色");
    // The failing provider's reason is shown verbatim, not hidden behind the last failure.
    expect(html).toContain("尚未登录");
    // A runnable fallback in the same chain is still shown as runnable.
    expect(html).toContain("可运行");
    // The blocked developer role is marked as having no runnable provider.
    expect(html).toContain("无可运行 Provider");
    // The guidance keeps the "no need to recreate the task" promise.
    expect(html).toContain("无需重新创建任务");
  });
});
