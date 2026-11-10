#!/usr/bin/env bash
# 只对本机已经安装的 CLI 尝试执行帮助命令；不会安装软件/修改配置。
# 某些 CLI 的 help 会启动交互或网络请求；建议在隔离环境运行。
set -u
base="${1:-./cli-help-snapshots}"
mkdir -p "$base"
commands=(claude codex agy gemini copilot agent cursor-agent kiro-cli vibe qwen kimi codebuddy grok mcode mimo qoder dsh trae-cli opencode aider goose)
for c in "${commands[@]}"; do
  if command -v "$c" >/dev/null 2>&1; then
    echo "Collecting $c help..."
    ("$c" --help < /dev/null) > "$base/$c.help.txt" 2>&1 || true
    ("$c" --version < /dev/null) > "$base/$c.version.txt" 2>&1 || true
  else
    echo "Skipping $c (not installed)"
  fi
done
printf 'Output: %s\n' "$base"
