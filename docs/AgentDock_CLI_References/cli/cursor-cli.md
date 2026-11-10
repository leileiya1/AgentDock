# Cursor CLI · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`agent / cursor-agent`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://cursor.com/docs/cli/overview
- **Headless / CI：** https://cursor.com/docs/cli/headless
- **旧版 CLI 参数表（已可能重定向）：** https://docs.cursor.com/en/cli/reference/parameters
- **ACP：** https://cursor.com/docs/cli/acp

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
agent
cursor-agent
cursor-agent login
cursor-agent logout
agent --help
```

## 3. 已核对的关键参数

**核对来源：** 官方历史参数页 + 当前文档导航。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 通用 | `-v, --version` | 输出版本 |
| 认证 | `-a, --api-key <key>` | API Key，或使用 CURSOR_API_KEY 环境变量 |
| 非交互 | `-p, --print` | 无交互输出；允许工具执行与文件修改 |
| 非交互 | `--output-format text\|json\|stream-json` | 与 --print 配套；旧版参考默认 stream-json |
| 会话 | `--resume [chatId]` | 恢复已有会话 |
| 模型 | `-m, --model <model>` | 使用指定模型 |
| 权限 | `-f, --force` | 允许命令，除被显式拒绝项外 |
| 界面 | `-b, --background` | 后台模式（版本相关） |
| 界面 | `--fullscreen` | 全屏模式 |
| 帮助 | `-h, --help` | 显示当前版本参数 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
agent -p "检查本项目的类型错误"
agent -p --output-format stream-json "分析代码"
agent --model MODEL
agent --resume SESSION_ID
```

## 5. 配置文件与生效范围

- 以 Cursor 当前 CLI 文档和工具实际 --help 为准。
- Headless API 与编辑器后台 Agent 不必共享同一会话协议。

## 6. AgentDock 集成建议

**推荐接入方式：** 独立配置 JSON 流事件与 Resume；先探测实际可执行文件名称。

**功能标签：** 交互 / 非交互 / stream-json / 会话 / 模型。

统一适配器至少保存以下字段：

```yaml
adapter_id: cursor-cli
executable: "agent"
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

- Cursor 旧版 `docs.cursor.com` 参数页现在可能重定向到新文档首页；这里保留可被检索到的原参考项，要求本机确认。
- 新版本既存在 `agent` 也可能提供 `cursor-agent` 命令；先使用 `command -v` 确认。
- `--force` 存在安全风险，应避免作为默认运行开关。
- **版本/限制：** 参数资料含可能被重定向的旧文档，属于需要运行时再次核对的条目。

## 8. 本机核对命令

```bash
agent --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
