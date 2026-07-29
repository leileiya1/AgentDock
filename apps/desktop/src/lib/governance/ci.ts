import type { CiCheck, CiCheckStatus } from "@/generated/bindings";

export const CI_STATUS_LABEL: Record<CiCheckStatus, string> = {
  pending: "运行中",
  passed: "通过",
  failed: "失败",
  cancelled: "已取消",
  skipped: "已跳过",
  unknown: "状态未知",
};

export function ciCheckSummary(check: CiCheck): string | null {
  if (check.status === "failed" || check.status === "cancelled") {
    return check.failureSummary ?? check.description ?? "检查没有提供失败摘要，请打开远端日志查看具体步骤。";
  }
  return check.description ?? null;
}

export function ciCounts(checks: CiCheck[]) {
  return checks.reduce(
    (counts, check) => {
      counts.total += 1;
      if (check.status === "passed") counts.passed += 1;
      else if (check.status === "failed" || check.status === "cancelled") counts.failed += 1;
      else if (check.status === "pending") counts.pending += 1;
      return counts;
    },
    { total: 0, passed: 0, failed: 0, pending: 0 }
  );
}

export function ciDuration(check: CiCheck): string | null {
  if (!check.startedAt || !check.completedAt) return null;
  const milliseconds = new Date(check.completedAt).getTime() - new Date(check.startedAt).getTime();
  if (!Number.isFinite(milliseconds) || milliseconds < 0) return null;
  const seconds = Math.round(milliseconds / 1000);
  if (seconds < 60) return `${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  return `${minutes} 分 ${String(seconds % 60).padStart(2, "0")} 秒`;
}
