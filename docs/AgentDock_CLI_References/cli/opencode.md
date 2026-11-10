# OpenCode · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：第三方开源　|　实际命令：`opencode`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://opencode.ai/docs/zh-cn/cli
- **服务端 API：** https://opencode.ai/docs/server/
- **配置：** https://opencode.ai/docs/config/
- **ACP：** https://opencode.ai/docs/acp/

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
opencode
opencode run
opencode serve
opencode attach
opencode web
opencode acp
opencode models
opencode session
opencode auth
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| TUI | `-c, --continue` | 继续上一会话 |
| TUI | `-s, --session <id>` | 使用指定会话 ID |
| TUI | `--fork` | 继续时分叉会话 |
| TUI | `-m, --model <provider/model>` | 指定 Provider/模型 |
| TUI | `--agent <name>` | 选择 Agent |
| TUI | `--port <number>` | 指定本地服务端口 |
| TUI | `--hostname <host>` | 指定主机绑定地址 |
| run | `opencode run <prompt>` | 无交互执行 |
| run | `--attach <url>` | 连接运行中的 opencode serve 服务 |
| run | `--format default\|json` | 默认格式或结构化 JSON 事件 |
| run | `--file <path>` | 附加文件 |
| run | `--dir <path>` | 任务目录 |
| run | `--variant <name>` | 模型专属变体（如思考强度） |
| run | `--thinking` | 展示思考块 |
| 协议 | `opencode acp` | 启动 ACP |
| 服务 | `opencode serve` | 运行后台 API 服务器 |
| 会话 | `opencode session` | 管理会话 |
| 模型 | `opencode models` | 查询可用模型 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
opencode run --model provider/model "复查代码"
opencode serve
opencode run --attach http://localhost:4096 "总结变更"
opencode run --format json "列出文件"
opencode acp
```

## 5. 配置文件与生效范围

- 配置文件按官方 config 文档设置。
- 多模型路由、Agent 权限和自定义工具均可以走 JSON 配置。

## 6. AgentDock 集成建议

**推荐接入方式：** serve / attach / ACP 适合长驻 Agent 连接；run 适合临时批任务。

**功能标签：** 交互 / 无交互 / JSON / Server / ACP / 会话 / 多模型。

统一适配器至少保存以下字段：

```yaml
adapter_id: opencode
executable: "opencode"
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

- OpenCode 的 JSON 事件、HTTP Server 和 ACP 是三个不同的接入面。
- `--model` 期望 provider/model 格式；不要用统一裸模型名。
- 默认仅本机监听；对外暴露 serve 时必须配置鉴权与访问控制。

## 8. 本机核对命令

```bash
opencode --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
