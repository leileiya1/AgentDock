import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { Project } from "@/generated/bindings";
import { ProjectOverview } from "./ProjectOverview";

const project: Project = {
  id: "p1",
  seq: 1,
  name: "AgentDock",
  repoPath: "/tmp/agentdock",
  defaultBranch: "main",
  worktreeRoot: "/tmp/worktrees",
  createdAt: "2026-07-29T00:00:00Z",
};

test("project overview renders a restrained project toolbar", () => {
  const html = renderToStaticMarkup(
    <ProjectOverview
      project={project}
      counts={{ all: 0, attention: 0, running: 0, done: 0 }}
      view="attention"
      onViewChange={() => {}}
      onNew={() => {}}
      onExport={() => {}}
      environmentSlot={<span>端到端环境就绪</span>}
      permissionSlot={<span>受限沙箱</span>}
      showStats={false}
    />,
  );

  expect(html.match(/<h1/g)?.length).toBe(1);
  expect(html).toContain("AgentDock");
  expect(html).toContain("main");
  expect(html).toContain("策略：按任务设置");
  expect(html).toContain("端到端环境就绪");
  expect(html).toContain("导出审计");
  expect(html).toContain("新建任务");
  expect(html).not.toContain("欢迎回来");
  expect(html).not.toContain("animate-sheen");
  expect(html).not.toContain("radial-gradient");
});

test("project summary exposes stable task filters without animated numbers", () => {
  const html = renderToStaticMarkup(
    <ProjectOverview
      project={project}
      counts={{ all: 16, attention: 13, running: 1, done: 2 }}
      view="attention"
      onViewChange={() => {}}
      onNew={() => {}}
      onExport={() => {}}
    />,
  );

  expect(html).toContain('role="tablist"');
  expect(html.match(/role="tab"/g)?.length).toBe(4);
  expect(html).toContain('aria-selected="true"');
  expect(html).toContain("运行中");
  expect(html).toContain(">13<");
  expect(html).not.toContain("text-[28px]");
});
