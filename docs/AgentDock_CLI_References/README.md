# AgentDock · 多厂商 Coding CLI 参数手册

> 文档整理日期：2026-11-06。共 **20** 个 CLI，各自独立 Markdown，非安装教程。

## 快速使用

在 `cli/` 中查找对应工具。每份包括：官方来源、子命令、核对后的重要参数、使用示例、配置与 AgentDock 集成建议、版本风险。

| 产品 | 文件 | 原始官方参数 / 参考页面 | 验证类型 |
|---|---|---|---|
| Claude Code | [cli/claude-code.md](cli/claude-code.md) | [官方资料](https://code.claude.com/docs/en/cli-reference) | 官方参数参考页 |
| Codex CLI | [cli/codex-cli.md](cli/codex-cli.md) | [官方资料](https://learn.chatgpt.com/docs/developer-commands?surface=cli) | 官方参数参考页 |
| Antigravity CLI | [cli/antigravity-cli.md](cli/antigravity-cli.md) | [官方资料](https://antigravity.google/docs/cli/reference/) | 官方参数参考页 |
| Gemini CLI | [cli/gemini-cli.md](cli/gemini-cli.md) | [官方资料](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/cli-reference.md) | 官方参数参考页 |
| GitHub Copilot CLI | [cli/github-copilot-cli.md](cli/github-copilot-cli.md) | [官方资料](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference) | 官方参数参考页 |
| Cursor CLI | [cli/cursor-cli.md](cli/cursor-cli.md) | [官方资料](https://cursor.com/docs/cli/overview) | 官方历史参数页 + 当前文档导航 |
| Kiro CLI | [cli/kiro-cli.md](cli/kiro-cli.md) | [官方资料](https://kiro.dev/docs/cli/reference/cli-commands/) | 官方参数参考页 |
| Mistral Vibe | [cli/mistral-vibe.md](cli/mistral-vibe.md) | [官方资料](https://docs.mistral.ai/vibe/code/cli/work-with-cli) | 官方参数参考页 |
| Qwen Code | [cli/qwen-code.md](cli/qwen-code.md) | [官方资料](https://qwenlm.github.io/qwen-code-docs/en/users/configuration/settings/) | 官方参数参考页 |
| Kimi Code CLI | [cli/kimi-code-cli.md](cli/kimi-code-cli.md) | [官方资料](https://www.kimi.com/code/docs/en/kimi-code-cli/reference/kimi-command) | 官方参数参考页 |
| CodeBuddy Code | [cli/codebuddy-code.md](cli/codebuddy-code.md) | [官方资料](https://www.codebuddy.cn/docs/cli/cli-reference) | 官方参数参考页 |
| Grok Build | [cli/grok-build.md](cli/grok-build.md) | [官方资料](https://docs.x.ai/build/cli/reference) | 官方参数参考页 |
| MiniMax MCode | [cli/minimax-mcode.md](cli/minimax-mcode.md) | [官方资料](https://github.com/MiniMax-AI/minimax-code) | 官方仓库公开 README（未找到完整独立 flags 手册） |
| MiMo Code | [cli/mimo-code.md](cli/mimo-code.md) | [官方资料](https://github.com/XiaomiMiMo/MiMo-Code) | 官方 README（仅部分 CLI 标志） |
| Qoder CLI | [cli/qoder-cli.md](cli/qoder-cli.md) | [官方资料](https://docs.qoder.com/zh/cli/cli-reference) | 官方参数参考页 |
| DeepSeek Harness | [cli/deepseek-harness.md](cli/deepseek-harness.md) | [官方资料](https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/cli-help.md) | 官方参数参考页 |
| Trae Agent | [cli/trae-agent.md](cli/trae-agent.md) | [官方资料](https://github.com/bytedance/trae-agent#-usage) | 官方参数参考页 |
| OpenCode | [cli/opencode.md](cli/opencode.md) | [官方资料](https://opencode.ai/docs/zh-cn/cli) | 官方参数参考页 |
| Aider | [cli/aider.md](cli/aider.md) | [官方资料](https://aider.chat/docs/config/options) | 官方参数参考页 |
| Goose | [cli/goose.md](cli/goose.md) | [官方资料](https://goose-docs.ai/docs/guides/goose-cli-commands/) | 官方参数参考页 |

## 什么是完整参数手册？

多数 CLI 在在线文档之外还不断增加新参数，官方有时不完整，`--help` 也可能有隐藏命令。因此本包是**可用作工程开发的核对版手册**，不是对所有版本“完整无遗漏”的保证。

## 推荐开发顺序

先接入 Claude Code、Codex、Qwen Code、Kimi Code、OpenCode 的非交互/服务端能力，接着接入支持 ACP 的 Kiro、CodeBuddy、Goose、MiniMax、Antigravity。余下产品通过可插拔 Adapter 逐步补齐。

## 关联文件

- [PARAMETER_MATRIX.md](PARAMETER_MATRIX.md)：统一能力/差异对照。
- [ADAPTER_DESIGN.md](ADAPTER_DESIGN.md)：调度器和适配器设计建议。
- [collect_cli_help.sh](collect_cli_help.sh)：本机非破坏式帮助文档收集脚本（仅对已经安装的命令）。
