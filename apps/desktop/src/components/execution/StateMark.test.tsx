import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { StateMark } from "./StateMark";

test("execution marks distinguish running, attention and failure", () => {
  expect(renderToStaticMarkup(<StateMark state="running" />)).toContain("text-status-running");
  expect(renderToStaticMarkup(<StateMark state="attention" />)).toContain("text-status-human");
  expect(renderToStaticMarkup(<StateMark state="failed" />)).toContain("text-status-danger");
});
