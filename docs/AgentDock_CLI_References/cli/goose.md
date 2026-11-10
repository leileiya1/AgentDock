# Goose · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：第三方开源　|　实际命令：`goose`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://goose-docs.ai/docs/guides/goose-cli-commands/
- **官网：** https://block.github.io/goose/
- **仓库：** https://github.com/block/goose

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
goose configure
goose info
goose run
goose session
goose session export
goose review
goose acp
goose serve
goose term
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 运行 | `goose run -t, --text <text>` | 无交互任务文本 |
| 运行 | `goose run -i, --instructions <file>` | 文件或 stdin 提示 |
| 运行 | `goose run --recipe <yaml>` | 执行 Recipe |
| 运行 | `goose run --params key=value` | 为 Recipe 传参数，可重复 |
| 运行 | `goose run --no-session` | 不保存会话 |
| 运行 | `goose run --max-turns <n>` | 限制最大轮次 |
| 运行 | `goose run --output-format text\|json\|stream-json` | 无交互输出格式 |
| 运行 | `goose run --provider <name>` | 当前运行 Provider |
| 运行 | `goose run --model <id>` | 当前运行 Model |
| 运行 | `goose run -q, --quiet` | 减少非结果日志 |
| 会话 | `goose session --resume` | 恢复会话 |
| 会话 | `goose session --session-id <id>` | 按 ID 选择会话 |
| 会话 | `goose session --fork` | 分叉会话 |
| 会话 | `goose session export --format json` | 导出会话 |
| 扩展 | `--with-extension <cmd>` | 添加 stdio 扩展 |
| 扩展 | `--with-streamable-http-extension <url>` | 添加 HTTP MCP 扩展 |
| 扩展 | `--with-builtin <name>` | 添加内置工具扩展 |
| 协议 | `goose acp` | 运行 ACP 服务端 |
| 服务 | `goose serve --host <host> --port <port>` | HTTP/WebSocket ACP 服务 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
goose run --text "分析依赖" --output-format stream-json
goose run --instructions ./task.md --max-turns 12
goose session --resume --session-id SESSION_ID
goose serve --host 127.0.0.1 --port 3284
goose acp
```

## 5. 配置文件与生效范围

- `goose configure` 选择 Provider 与 Extensions。
- 与远程 ACP 服务配套需设置 `GOOSE_SERVER__SECRET_KEY`。

## 6. AgentDock 集成建议

**推荐接入方式：** run JSON 流适合工作流，ACP 服务适合长期会话。

**功能标签：** 交互 / run / stream-json / session / ACP / Server / 扩展 / Recipe。

统一适配器至少保存以下字段：

```yaml
adapter_id: goose
executable: "goose"
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

- Goose `goose serve` 需要服务端鉴权，不能默认暴露到公网。
- Run 的 `--output-format` 有 text/json/stream-json，适合做统一 Runner。
- 可用 Recipe 参数化复杂任务，比临时拼接长提示词更可靠。

## 8. 本机核对命令

```bash
goose --help
goose run --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
