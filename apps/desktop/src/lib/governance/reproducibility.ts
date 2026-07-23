import type {
  QualityReplayStatus,
  ReproducibilityDriftKind,
  ReproducibilityLevel,
} from "@/generated/bindings";

export const REPRO_LEVEL: Record<ReproducibilityLevel, { label: string; detail: string }> = {
  fixed_commit: {
    label: "固定提交",
    detail: "代码和验证命令已固定，但工具、系统与外部依赖可能变化。结果只能证明同一提交被再次验证。",
  },
  environment_locked: {
    label: "环境锁定",
    detail: "工具、环境变量摘要和外部依赖快照必须匹配；检测到漂移会在运行前停止。",
  },
  hermetic: {
    label: "密闭复现",
    detail: "验证使用摘要固定的容器与依赖，复现可信度最高；仍需关注清单列出的限制。",
  },
};

export const REPLAY_STATUS: Record<QualityReplayStatus, string> = {
  succeeded: "复验通过",
  validation_failed: "复验未通过",
  drift_blocked: "环境漂移，已停止",
  failed: "复验基础设施失败",
};

export const DRIFT_INFO: Record<ReproducibilityDriftKind, { label: string; recovery: string }> = {
  tool_versions: { label: "工具版本变化", recovery: "切回原工具版本，或在新 revision 重新生成清单。" },
  environment_variables: { label: "环境变量变化", recovery: "在原执行节点恢复变量；界面只显示变量名，不显示值。" },
  system_dependencies: { label: "系统依赖变化", recovery: "恢复验证命令依赖的版本，或改用原执行节点。" },
  container_images: { label: "容器镜像变化", recovery: "拉取清单中的 digest，不能只使用可变 tag。" },
  git_submodules: { label: "子模块提交变化", recovery: "同步并检出清单记录的子模块提交。" },
  git_lfs_objects: { label: "Git LFS 对象变化", recovery: "执行 LFS fetch/checkout，并确认对象完整。" },
  external_dependencies: { label: "外部依赖快照变化", recovery: "恢复原数据库/服务快照，或为新状态建立新 revision。" },
};

export function scoreDeltaLabel(delta: number | null) {
  if (delta === null) return "无法比较";
  if (delta === 0) return "与原验证一致";
  return delta > 0 ? `比原验证高 ${delta} 分` : `比原验证低 ${Math.abs(delta)} 分`;
}
