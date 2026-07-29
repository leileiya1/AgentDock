import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ValidationReportCard } from "./ValidationReportCard";
import type { TestReport } from "@/lib/execution/testReport";

const s = (over: Partial<TestReport["steps"][number]>) => ({
  name: "step",
  argv: [],
  exitCode: 0,
  durationMs: 1000,
  stdoutTail: "",
  stderrTail: "",
  ...over,
});

describe("ValidationReportCard", () => {
  it("shows a passing run with per-step counts (§18/§19)", () => {
    const report: TestReport = { passed: true, steps: [s({ name: "build" }), s({ name: "unit" })] };
    const html = renderToStaticMarkup(<ValidationReportCard report={report} />);
    expect(html).toContain("验证通过");
    expect(html).toContain("2 步");
    expect(html).toContain("2 通过");
    expect(html).toContain("build");
  });

  it("distinguishes a timeout from an assertion failure and shows the main error", () => {
    const report: TestReport = {
      passed: false,
      steps: [
        s({ name: "build", exitCode: 0 }),
        s({ name: "slow", exitCode: null, durationMs: 900_000 }),
        s({ name: "unit", exitCode: 1, stderrTail: "assertion failed at auth.rs:42" }),
      ],
    };
    const html = renderToStaticMarkup(<ValidationReportCard report={report} />);
    expect(html).toContain("验证未通过");
    expect(html).toContain("超时"); // the null-exit step
    expect(html).toContain("退出码 1"); // the assertion failure
    expect(html).toContain("assertion failed at auth.rs:42"); // main error surfaced
  });
});
