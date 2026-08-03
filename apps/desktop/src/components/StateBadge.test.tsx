import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { StateBadge } from "./StateBadge";

test("running and human states have distinct semantic channels", () => {
  const running = renderToStaticMarkup(<StateBadge status="DEVELOPING" />);
  const human = renderToStaticMarkup(<StateBadge status="WAITING_FOR_HUMAN_APPROVAL" />);

  expect(running).toContain("text-status-running");
  expect(running).toContain("animate-pulse-dot");
  expect(human).toContain("text-status-human");
  expect(human).not.toContain("animate-pulse-dot");
  expect(human).toContain("等你批准");
});
