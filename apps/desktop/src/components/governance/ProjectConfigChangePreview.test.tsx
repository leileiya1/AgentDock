import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { ProjectConfigTrust } from "@/generated/bindings";
import { ProjectConfigChangePreview } from "./ProjectConfigChangePreview";

const value: ProjectConfigTrust = {
  exists: true, path: "/repo/.agentflow/project.toml", sha256: "new", trusted: false,
  validationSteps: ["tests"], extraAllowedCommands: ["bun test"],
  validationCommands: [{ name: "tests", argv: ["bun", "test"], timeoutSecs: 60 }],
  environmentAllowlist: ["CI"], externalDependencies: ["fixture-db"], containerImages: [],
  lockEnvironment: true, hermetic: false, previousApprovedSha256: "old",
  changes: [{ path: "agents.extra_allowed_commands", kind: "changed", before: "[\"bun test\"]", after: "[\"bun test --coverage\"]", highRisk: true }],
  byteOnlyChange: false, approvedAt: "now",
};

describe("ProjectConfigChangePreview", () => {
  it("shows semantic diff and exact command permission before approval", () => {
    const html = renderToStaticMarkup(<ProjectConfigChangePreview value={value} />);
    expect(html).toContain("相对上次批准的配置变化");
    expect(html).toContain("Agent 额外命令权限");
    expect(html).toContain("bun test --coverage");
    expect(html).toContain("验证 · tests");
    expect(html).toContain("fixture-db");
  });
});
