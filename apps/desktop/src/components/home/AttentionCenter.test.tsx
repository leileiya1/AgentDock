import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { TaskSummary } from "@/generated/bindings";
import { AttentionCenter } from "./AttentionCenter";

test("attention center names the reason and the next step", () => {
  const task = {
    id: "t1",
    projectId: "p1",
    seq: 12,
    title: "恢复真实运行",
    status: "BLOCKED",
    blockedReason: "auth_expired",
    currentRevision: 1,
    developerAgent: "qoder_cli",
    reviewerAgent: "grok_cli",
    updatedAt: "2026-07-29T00:00:00Z",
  } satisfies TaskSummary;
  const html = renderToStaticMarkup(<AttentionCenter tasks={[task]} onOpen={() => {}} />);

  expect(html).toContain("需要你处理");
  expect(html).toContain("登录或密钥已失效");
  expect(html).toContain("下一步：");
  expect(html).toContain("查看恢复方式");
});
