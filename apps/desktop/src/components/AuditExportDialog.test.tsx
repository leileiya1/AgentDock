import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { AuditExportResult } from "@/generated/bindings";
import { AuditExportSummary } from "./AuditExportDialog";

const result: AuditExportResult = {
  path: "/Users/test/AgentFlow/exports/audit-project.jsonl",
  scope: "project",
  projectId: "p1",
  taskId: null,
  eventCount: 42,
  taskCount: 3,
  bytes: 2048,
  redacted: true,
  createdAt: "2026-07-20T12:00:00Z",
};

describe("AuditExportSummary", () => {
  it("shows scope counts, size and the exact local save path", () => {
    const html = renderToStaticMarkup(<AuditExportSummary result={result} />);
    expect(html).toContain("导出完成");
    expect(html).toContain("42 条事件");
    expect(html).toContain("3 个任务");
    expect(html).toContain("2.0 KB");
    expect(html).toContain("audit-project.jsonl");
  });
});
