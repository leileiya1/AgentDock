import type { ProjectConfigChangeKind } from "@/generated/bindings";

export const CONFIG_CHANGE_LABEL: Record<ProjectConfigChangeKind, string> = {
  added: "新增",
  removed: "删除",
  changed: "修改",
};

export function configPathLabel(path: string) {
  const labels: Record<string, string> = {
    "validate.steps": "验证命令",
    "agents.extra_allowed_commands": "Agent 额外命令权限",
    "reproducibility.env_allowlist": "允许记录摘要的环境变量",
    "reproducibility.external_dependencies": "外部依赖快照",
    "reproducibility.container_images": "容器镜像 digest",
    "reproducibility.lock_environment": "环境锁定",
    "reproducibility.hermetic": "密闭复现",
    "review.exclude_globs": "审查排除路径",
    "review.max_patch_bytes": "审查最大 Diff 大小",
  };
  return labels[path] ?? path;
}

export function displayConfigValue(value: string | null) {
  if (value === null) return "（无）";
  try {
    const parsed = JSON.parse(value);
    return typeof parsed === "string" ? parsed : JSON.stringify(parsed, null, 2);
  } catch {
    return value;
  }
}
