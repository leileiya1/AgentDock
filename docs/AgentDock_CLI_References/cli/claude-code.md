# Claude Code · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`claude`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://code.claude.com/docs/en/cli-reference
- **Settings / 配置：** https://code.claude.com/docs/en/settings
- **Headless / SDK：** https://code.claude.com/docs/en/headless
- **权限模式：** https://code.claude.com/docs/en/permissions

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
claude
claude -p "问题"
claude -c
claude -r SESSION_ID
claude auth status
claude update
claude mcp
claude agents
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 会话 | `-c, --continue` | 继续工作目录中最近会话 |
| 会话 | `-r, --resume <id\|name>` | 恢复指定历史会话；省略 ID 可使用选择器 |
| 会话 | `--fork-session` | 在恢复会话时分叉出新的会话 ID |
| 会话 | `--session-id <UUID>` | 新会话使用指定 UUID |
| 工作区 | `--add-dir <path>` | 附加可访问目录；并不自动加载其完整项目配置 |
| 模型 | `--model <name>` | 当前会话的模型 |
| 模型 | `--fallback-model <list>` | 主模型不可用时按顺序尝试备用模型 |
| 模型 | `--effort <level>` | 设置推理强度，支持取值依模型而变 |
| 非交互 | `-p, --print` | 执行单轮或脚本任务后退出 |
| 非交互 | `--output-format text\|json\|stream-json` | 非交互输出格式 |
| 非交互 | `--input-format text\|stream-json` | 非交互标准输入格式 |
| 非交互 | `--include-partial-messages` | 流式 JSON 中包含增量消息；需 -p/stream-json |
| 非交互 | `--json-schema <json>` | 要求最终结构化 JSON 输出；print 模式 |
| 预算 | `--max-budget-usd <amount>` | 为本次 print 运行设置费用估算上限 |
| 权限 | `--permission-mode <mode>` | 如 default/acceptEdits/plan/dontAsk/bypassPermissions；可用性按版本 |
| 权限 | `--allowedTools <rules>` | 预先放行指定工具调用 |
| 权限 | `--dangerously-skip-permissions` | 跳过权限确认：仅适用隔离环境 |
| Agent | `--agent <name>` | 使用指定主 Agent |
| Agent | `--agents <json>` | 动态定义子 Agent |
| Prompt | `--append-system-prompt <text>` | 追加系统指令 |
| Prompt | `--append-system-prompt-file <file>` | 从文件追加系统指令 |
| 隔离 | `--bare` | 简化启动，跳过自动发现的多种扩展 |
| 隔离 | `--restricted` | 限制访问主机环境和工具（依版本） |
| 诊断 | `--verbose` | 详细日志；流式分析常搭配使用 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
claude --model sonnet --effort medium
claude -p "仅总结这次改动" --output-format json
claude -p "列出改动文件" --output-format stream-json --verbose
claude --permission-mode plan
claude --resume SESSION_ID --fork-session
```

## 5. 配置文件与生效范围

- 用户级/项目级 settings.json；权限与 Hooks 规则以官方 settings 文档为准。
- CLI 参数覆盖部分本地默认设置，但不应绕过组织托管策略。

## 6. AgentDock 集成建议

**推荐接入方式：** 首选流式 JSON + SDK；保留原始 message 和 tool_use_id；权限按 action 审批。

**功能标签：** 交互 / 单次任务 / JSON / 流事件 / 会话恢复 / 权限 / 子 Agent / MCP。

统一适配器至少保存以下字段：

```yaml
adapter_id: claude-code
executable: "claude"
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

- stream-json 不是别的厂商 JSONL 事件的通用协议；应解析 message、tool、result 等事件并记录原始数据。
- `--allowedTools` 是放行规则，不等价于禁止其它工具；要限制工具范围另查 `--tools`。
- `--max-budget-usd` 是客户端估算上限，不可当成严格计费封顶。
- **版本/限制：** 含新增标志的最低版本限制；`claude --help` 不保证列出所有参数。

## 8. 本机核对命令

```bash
claude --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
