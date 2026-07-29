import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { RunFailureDetail } from "./RunFailureDetail";

describe("RunFailureDetail", () => {
  it("shows which agent failed, the exit code, and the safe project state (§16)", () => {
    const html = renderToStaticMarkup(
      <RunFailureDetail failure={{ role: "developer", agent: "claude_code", kind: "exit", exitCode: 1 }} />
    );
    expect(html).toContain("开发 Agent");
    expect(html).toContain("退出代码 1");
    // The project-safety line must be honest: on total failure the round is rolled back.
    expect(html).toContain("已回滚");
    expect(html).toContain("未受影响");
  });

  it("frames a hang as a timeout rather than an exit code", () => {
    const html = renderToStaticMarkup(
      <RunFailureDetail failure={{ role: "reviewer", agent: "codex", kind: "timeout", exitCode: null }} />
    );
    expect(html).toContain("审查 Agent");
    expect(html).toContain("运行超时");
    expect(html).not.toContain("退出代码");
  });

  it("does not claim a rollback for read-only reviewer/planner runs", () => {
    const reviewer = renderToStaticMarkup(
      <RunFailureDetail failure={{ role: "reviewer", agent: "codex", kind: "exit", exitCode: 1 }} />
    );
    // A read-only review changes nothing, so there is no round to "roll back".
    expect(reviewer).toContain("只读");
    expect(reviewer).not.toContain("已回滚");

    const developer = renderToStaticMarkup(
      <RunFailureDetail failure={{ role: "developer", agent: "codex", kind: "exit", exitCode: 1 }} />
    );
    expect(developer).toContain("已回滚");
    expect(developer).not.toContain("只读");
  });
});
