# 多厂商 CLI 能力与参数对照

> 日期：2026-11-06。仅比较手册已确认或已注明的能力；`—` 表示本次未核实，不代表产品不支持。

| 产品 | 交互入口 | 非交互入口 | 输出格式 | 会话 | 服务/协议 |
|---|---|---|---|---|---|
| Claude Code | `claude` | `claude -p` | `text/json/stream-json` | `-c/-r` | `SDK/stream-json` |
| Codex CLI | `codex` | `codex exec` | `JSONL (--json)` | `exec resume` | `app-server` |
| Antigravity CLI | `agy` | `agy -p` | `text/json/stream-json` | `--conversation` | `ACP` |
| Gemini CLI | `gemini` | `gemini -p` | `需按版本核对` | — | `ACP 版本相关` |
| GitHub Copilot CLI | `copilot` | `copilot -p` | `text/JSONL` | `--resume` | — |
| Cursor CLI | `agent` | `agent -p` | `text/json/stream-json` | `--resume` | `ACP / Headless` |
| Kiro CLI | `kiro-cli chat` | `chat --no-interactive` | `text / 版本相关` | `--resume-id` | `acp` |
| Mistral Vibe | `vibe` | `vibe -p` | `text/json/streaming` | `以配置/命令核对` | — |
| Qwen Code | `qwen` | `qwen -p` | `text/json/stream-json` | `会话/Daemon` | `Daemon / ACP` |
| Kimi Code CLI | `kimi` | `kimi -p` | `text/stream-json` | `-c/-S` | `acp` |
| CodeBuddy Code | `codebuddy` | `codebuddy -p` | `text/json/stream-json` | `-c/-r` | `--acp/--serve/--a2a` |
| Grok Build | `grok` | `参见 Headless` | `需核对` | `-r/-c` | `Headless` |
| MiniMax MCode | `mcode` | `mcode exec` | `需核对` | `--continue/--session` | `acp/SDK` |
| MiMo Code | `mimo` | `mimo run` | `需核对` | `需核对` | — |
| Qoder CLI | `qoder` | `提示词或 --print` | `需核对` | `需核对` | `ACP` |
| DeepSeek Harness | `dsh web` | `dsh headless` | `Profile 相关` | `Profile 相关` | `dsh acp/sdk` |
| Trae Agent | `trae-cli interactive` | `trae-cli run` | `轨迹 JSON` | `需核对` | — |
| OpenCode | `opencode` | `opencode run` | `default/json` | `--session` | `serve/acp` |
| Aider | `aider` | `--message` | `普通终端输出` | `Git-aware` | — |
| Goose | `goose session` | `goose run` | `text/json/stream-json` | `--resume` | `acp/serve` |

## 必须区分的差异

1. **JSON 与 JSONL 不是同义词。** 一次性 JSON、逐行事件 JSONL、流式 token delta 的边界与 schema 往往不同。
2. **相同的参数名不等于相同的权限模式。** 例如 `--yolo` 在 Kimi 的解释就与其它 CLI 不同。
3. **恢复会话要绑定工作目录和厂商版本。** Session ID 不可直接跨进程 / 厂商复用。
4. **ACP Server ≠ HTTP API ≠ PTY。** 需要按协议分别解析。
