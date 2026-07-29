import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { CloseGuardDialog } from "./CloseGuard";

describe("CloseGuardDialog", () => {
  it("states how many tasks run and that they continue in the background (§40)", () => {
    const html = renderToStaticMarkup(
      <CloseGuardDialog open activeCount={2} onKeepRunning={() => {}} onCancel={() => {}} />
    );
    expect(html).toContain("关闭 AgentFlow？");
    expect(html).toContain("2 个任务正在运行");
    expect(html).toContain("后台继续");
    expect(html).toContain("取消关闭");
    // We do not support true pause-and-save for running tasks, so that option must not appear.
    expect(html).not.toContain("暂停并保存");
  });

  it("renders nothing when closed", () => {
    const html = renderToStaticMarkup(
      <CloseGuardDialog open={false} activeCount={0} onKeepRunning={() => {}} onCancel={() => {}} />
    );
    expect(html).toBe("");
  });
});
