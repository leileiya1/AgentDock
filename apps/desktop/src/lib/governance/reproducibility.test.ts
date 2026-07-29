import { describe, expect, it } from "bun:test";
import { DRIFT_INFO, REPRO_LEVEL, scoreDeltaLabel } from "./reproducibility";

describe("reproducibility copy", () => {
  it("does not overstate fixed-commit replay and gives actionable drift recovery", () => {
    expect(REPRO_LEVEL.fixed_commit.detail).toContain("工具、系统与外部依赖可能变化");
    expect(DRIFT_INFO.external_dependencies.recovery).toContain("快照");
    expect(scoreDeltaLabel(-8)).toBe("比原验证低 8 分");
    expect(scoreDeltaLabel(0)).toBe("与原验证一致");
  });
});
