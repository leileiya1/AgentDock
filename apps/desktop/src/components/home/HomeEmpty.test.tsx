import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { HomeEmpty } from "./HomeEmpty";

test("home empty state states the fact and one next action", () => {
  const html = renderToStaticMarkup(<HomeEmpty onNew={() => {}} />);

  expect(html).toContain("还没有任务");
  expect(html).toContain("新建任务");
  expect(html.match(/<button/g)?.length).toBe(1);
  expect(html).not.toContain("Sparkles");
  expect(html).not.toContain("多智能体协作");
  expect(html).not.toContain("radial-gradient");
  expect(html).not.toContain("animate-pulse-dot");
});
