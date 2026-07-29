import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { QualityEvaluation, QualityReplayAttempt, ReproducibilityManifest } from "@/generated/bindings";
import { ReproducibilityPanel } from "./ReproducibilityPanel";

const quality: QualityEvaluation = { taskId: "t1", revision: 1, score: 92, grade: "A", passed: true, replay: false, checks: [], createdAt: "2026-07-20T09:00:00Z" };
const manifest: ReproducibilityManifest = {
  taskId: "t1", revision: 1, commitSha: "commit", manifestSha256: "manifest",
  environment: { validation_location: "local", validation_platform: "macOS" },
  reproducibilityLevel: "environment_locked", limitations: [], inputSha256: "input",
  patchSha256: "patch", validationConfigSha256: "validation", createdAt: "2026-07-20T09:00:00Z",
};
const attempt: QualityReplayAttempt = {
  id: "a1", taskId: "t1", revision: 1, status: "drift_blocked", reproducibilityLevel: "environment_locked",
  environmentMatch: false, drift: [{ kind: "external_dependencies", changedKeys: ["fixture-db"] }],
  originalQuality: quality, replayQuality: null, scoreDelta: null, errorCode: "REPRODUCIBILITY_DRIFT",
  errorDetail: null, createdAt: "2026-07-20T10:00:00Z", finishedAt: "2026-07-20T10:00:01Z",
};

describe("ReproducibilityPanel", () => {
  it("shows trust level, original/replay distinction, drift and recovery", () => {
    const html = renderToStaticMarkup(<ReproducibilityPanel manifest={manifest} originalQuality={quality} latestReplay={attempt} />);
    expect(html).toContain("环境锁定");
    expect(html).toContain("原验证");
    expect(html).toContain("环境漂移，已停止");
    expect(html).toContain("fixture-db");
    expect(html).toContain("恢复建议");
  });
});
