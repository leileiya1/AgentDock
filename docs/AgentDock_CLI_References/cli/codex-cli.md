# Codex CLI · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`codex`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://learn.chatgpt.com/docs/developer-commands?surface=cli
- **Config reference：** https://learn.chatgpt.com/docs/config-file/config-reference
- **Non-interactive：** https://developers.openai.com/codex/non-interactive
- **App Server：** https://developers.openai.com/codex/app-server

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
codex
codex exec
codex exec resume
codex exec fork
codex resume
codex fork
codex app-server
codex features list
codex mcp
codex doctor
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 通用 | `-m, --model <name>` | 为本次命令选择模型 |
| 通用 | `-C, --cd <dir>` | 工作目录 |
| 通用 | `-c, --config key=value` | 覆盖 config.toml 的单一设置，可重复 |
| 通用 | `-p, --profile <name>` | 叠加配置 Profile（注意不同子命令语义） |
| 通用 | `--add-dir <dir>` | 追加工作区可写路径 |
| 安全 | `-s, --sandbox <policy>` | read-only / workspace-write / danger-full-access |
| 安全 | `-a, --ask-for-approval <policy>` | on-request / never |
| 安全 | `--dangerously-bypass-approvals-and-sandbox` | 无审批且无沙箱；高风险 |
| 安全 | `--approve-for-me` | 自动审核授权方式，注意互斥选项 |
| 运行 | `--worktree` | 在托管 Git worktree 中启动（受命令限制） |
| 运行 | `--strict-config` | 对未知配置键直接报错 |
| 运行 | `--enable <feature>` | 本次启用实验特性 |
| 运行 | `--disable <feature>` | 本次禁用实验特性 |
| 运行 | `--oss` | 启用本地开源模型提供方 |
| 运行 | `--local-provider <provider>` | 如 ollama / lmstudio，配合 --oss |
| exec | `codex exec --json` | 非交互执行并输出 JSONL 事件 |
| exec | `codex exec -o <file>` | 将最终回答写入文件 |
| exec | `codex exec --output-schema <file>` | 结构化最终输出 JSON Schema 文件 |
| exec | `codex exec --ephemeral` | 不持久化本轮 rollout 文件 |
| exec | `codex exec --skip-git-repo-check` | 非 Git 仓库运行 |
| exec | `codex exec resume <id>` | 恢复指定 exec 会话 |
| exec | `codex exec resume --last` | 恢复当前工作区最近会话 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
codex exec --json "梳理依赖关系并输出结论"
codex exec -m MODEL -s workspace-write "修改测试并运行"
codex exec --output-schema ./schema.json "生成变更摘要"
codex exec resume --last "继续修复上次问题"
codex -c model_reasoning_effort="medium"
```

## 5. 配置文件与生效范围

- 主配置 `~/.codex/config.toml`（或 `$CODEX_HOME/config.toml`）。
- 使用 `-c model_reasoning_effort="medium"` 等方式一次性覆盖 TOML 字段；以当前 schema 为准。
- 批准策略和 sandbox 是独立维度。

## 6. AgentDock 集成建议

**推荐接入方式：** 优先 App Server 做长会话；CLI exec 用 JSONL 作批处理；推荐进程按工作区隔离。

**功能标签：** 交互 / exec / JSONL / 会话恢复 / worktree / 沙箱 / App Server / MCP。

统一适配器至少保存以下字段：

```yaml
adapter_id: codex-cli
executable: "codex"
version: <本机 --version 结果>
cwd: <workspace absolute path>
mode: <interactive | headless | server | acp>
session_id: <按厂商存储，不能跨厂商通用>
model: <厂商原生模型标识>
approval_policy: <厂商原生值>
output_parser: <按厂商协议选择>
```

建议将 stdout 与 stderr 分开采集，保留原始事件，同时转成 AgentDock 内部的 `text_delta`、`tool_start`、`tool_end`、`approval_request`、`session_state` 和 `final_result` 等事件；这些**是建议的 AgentDock 抽象**，不是厂商都支持的同名字段。

## 7. 易错点 / 已知限制

- Codex 的推理强度主要走 config.toml 的 `model_reasoning_effort` / `-c`，不应假定它提供 Claude 同名 `--effort`。
- `--json` 在 `codex exec` 输出 JSONL 事件，而不是一次性 JSON。
- App Server 接口比直接 PTY 抓屏更适合长期服务化接入；应按协议处理交互和批准。
- **版本/限制：** `--worktree` 并非所有子命令都支持；不能与某些 resume/ephemeral 组合。

## 8. 本机核对命令

```bash
codex --help
codex exec --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
