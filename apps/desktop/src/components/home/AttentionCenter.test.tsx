import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { TaskSummary } from "@/generated/bindings";
import { AttentionCenter } from "./AttentionCenter";

function task(seq: number, status: TaskSummary["status"] = "BLOCKED"): TaskSummary {
  return {
    id: `t${seq}`,
    projectId: "p1",
    seq,
    title: `恢复真实运行 ${seq}`,
    status,
    blockedReason: status === "BLOCKED" ? "auth_expired" : null,
    currentRevision: 1,
    developerAgent: "qoder_cli",
    reviewerAgent: "grok_cli",
    updatedAt: `2026-07-29T${String(seq).padStart(2, "0")}:00:00Z`,
  };
}

test("attention center renders one concise reason and one action", () => {
  const html = renderToStaticMarkup(<AttentionCenter tasks={[task(1)]} onOpen={() => {}} />);

  expect(html).toContain("需要你处理");
  expect(html).toContain("Agent 登录已失效，任务已安全暂停");
  expect(html).toContain("恢复任务");
  expect(html).toContain("更新于");
  expect(html).not.toContain("下一步：");
  expect(html).not.toContain("查看恢复方式");
  expect(html.match(/role="button"/g)?.length ?? 0).toBe(0);
});

test("attention center limits the initial queue and offers a truthful total", () => {
  const html = renderToStaticMarkup(
    <AttentionCenter tasks={Array.from({ length: 10 }, (_, index) => task(index + 1))} onOpen={() => {}} />,
  );

  expect(html.match(/恢复真实运行/g)?.length).toBe(8);
  expect(html).toContain("查看全部 10 项");
  expect(html).toContain('tabindex="0"');
  expect(html).toContain('tabindex="-1"');
});
