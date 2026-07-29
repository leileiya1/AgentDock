import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { StopConfirmDialog, STOP_PRESERVED_ITEMS } from "./StopRunButton";

describe("StopConfirmDialog", () => {
  it("asks to stop, lists what is kept, and is honest about what is discarded (§17)", () => {
    const html = renderToStaticMarkup(
      <StopConfirmDialog open pending={false} onClose={() => {}} onConfirm={() => {}} />
    );
    expect(html).toContain("停止当前正在运行的 Agent？");
    expect(html).toContain("继续运行");
    expect(html).toContain("停止任务");
    // Every genuinely-preserved item is surfaced.
    for (const item of STOP_PRESERVED_ITEMS) {
      expect(html).toContain(item);
    }
    // The copy must NOT overpromise: cancel is terminal and force-cleans the worktree,
    // so it cannot claim in-progress files survive or that the task resumes from here.
    expect(html).toContain("尚未提交的改动会被丢弃");
    expect(html).toContain("已取消");
    expect(html).not.toContain("可恢复");
    expect(html).not.toContain("从当前状态继续");
  });

  it("reflects the pending state while the stop is in flight", () => {
    const html = renderToStaticMarkup(
      <StopConfirmDialog open pending onClose={() => {}} onConfirm={() => {}} />
    );
    expect(html).toContain("停止中…");
    expect(html).toContain("disabled");
  });

  it("renders nothing when closed", () => {
    const html = renderToStaticMarkup(
      <StopConfirmDialog open={false} pending={false} onClose={() => {}} onConfirm={() => {}} />
    );
    expect(html).toBe("");
  });
});
