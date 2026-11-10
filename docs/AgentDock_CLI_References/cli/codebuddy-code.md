# CodeBuddy Code · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`codebuddy`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://www.codebuddy.cn/docs/cli/cli-reference
- **配置/Agent：** https://www.codebuddy.cn/docs/cli/
- **安装（用于定位版本）：** https://www.codebuddy.cn/docs/cli/installation

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
codebuddy
codebuddy -p
codebuddy --serve
codebuddy --acp
codebuddy --a2a
codebuddy update
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 会话 | `-c, --continue` | 继续最近对话 |
| 会话 | `-r, --resume <id>` | 通过会话 ID 恢复 |
| 非交互 | `-p, --print` | 执行一次查询后退出 |
| 输出 | `--output-format text\|json\|stream-json` | 适配一次性或流事件输出 |
| 输入 | `--input-format text\|stream-json` | 可通过 stdin 持续交互 |
| 输出 | `--include-partial-messages` | 流式增量事件 |
| 结构化 | `--json-schema <json>` | 按 schema 验证最终输出 |
| Agent | `--agent <name>` | 选择主 Agent；multitask 仅 TUI 场景 |
| Agent | `--agents <json>` | 通过 JSON 动态声明子代理 |
| Agent | `--multitask` | 仅交互式 TUI 的多任务 overlay |
| 模型 | `--model <name>` | 设置模型 |
| 限制 | `--max-turns <n>` | 控制非交互轮次上限 |
| Prompt | `--system-prompt-file <file>` | 打印模式替换系统 Prompt |
| Prompt | `--append-system-prompt <text>` | 追加系统指令 |
| 工具 | `--allowedTools <rules>` | 预先放行工具 |
| 服务 | `--serve` | HTTP Web/API/ACP 服务 |
| 服务 | `--port <n>` | HTTP 服务端口 |
| 服务 | `--host <host>` | 服务绑定地址 |
| 协议 | `--acp` | 以 ACP 服务端启动；默认 stdio |
| 协议 | `--acp-transport <mode>` | stdio / streamable-http |
| 协议 | `--a2a` | 运行 A2A 协议入口，与 ACP 冲突 |
| 后台 | `--bg` | 后台会话运行 |
| 预热 | `--prewarm` | 启动后预热待命，等 IPC 唤醒 |
| 追踪 | `--startup-prof-md` | 采集启动性能报告 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
codebuddy -p "说明项目结构" --output-format json
codebuddy --acp
codebuddy --serve --host 127.0.0.1 --port 7890
codebuddy -p "检查代码" --max-turns 3
codebuddy --agents '{"reviewer":{"description":"审查","prompt":"只做审查"}}'
```

## 5. 配置文件与生效范围

- 从 CodeBuddy docs CLI reference 获取完整配置项；有代理、prewarm、worktree 等专门章节。
- 输出完整工具事件前请确认不会泄露 Secret 或工作区隐私。

## 6. AgentDock 集成建议

**推荐接入方式：** ACP 与 A2A 分别实现适配器；保留 serve 的独立认证配置。

**功能标签：** 交互 / Headless / JSON / stream-json / Agent / ACP / A2A / HTTP / 预热。

统一适配器至少保存以下字段：

```yaml
adapter_id: codebuddy-code
executable: "codebuddy"
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

- `--multitask` 只支持真实交互式 TUI，不能与 `-p`、ACP、serve 组合。
- `--a2a` 与 `--acp` 冲突。
- `--serve --host 0.0.0.0` 涉及鉴权风险，默认应绑定 127.0.0.1。
- `--acp` 与 `--serve` 提供不同的 ACP 承载方式。

## 8. 本机核对命令

```bash
codebuddy --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
