# MiniMax MCode · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`mcode`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://github.com/MiniMax-AI/minimax-code
- **Headless/ACP 仓库说明：** https://github.com/MiniMax-AI/minimax-code#work-in-your-own-project
- **TypeScript SDK：** https://www.npmjs.com/package/@minimax-ai/code-sdk

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
mcode
mcode exec
mcode acp
mcode init .
/sessions
/help
/compact
```

## 3. 已核对的关键参数

**核对来源：** 官方仓库公开 README（未找到完整独立 flags 手册）。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 模式 | `--mode standard\|lightweight` | 标准工作模式和轻量聊天模式；lightweight 无文件工具 |
| 会话 | `--continue` | 继续当前工作区最近会话 |
| 会话 | `--session` | 进入会话选择器 |
| 命令 | `mcode exec <prompt>` | 无交互执行 |
| 命令 | `mcode acp` | 启动 ACP 集成 |
| 命令 | `mcode init .` | 初始化项目 AGENTS.md |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
mcode "修复编译失败"
mcode exec "分析最近变更"
mcode exec --mode lightweight "说明 CAP 定理"
mcode --continue
mcode acp
```

## 5. 配置文件与生效范围

- Provider 存在 config.yaml 的 custom_provider 配置。
- MCode 支持 TypeScript SDK，用它可以控制启动、事件流和恢复。

## 6. AgentDock 集成建议

**推荐接入方式：** exec/ACP/SDK 可分别选择；lightweight 模式不能编辑仓库。

**功能标签：** 交互 / Headless / ACP / SDK / 会话。

统一适配器至少保存以下字段：

```yaml
adapter_id: minimax-mcode
executable: "mcode"
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

- 轻量模式不加载工具、技能、MCP，不能读写项目文件。
- 轻量模式不能与 `--session` / `--continue` 组合。
- 未找到独立的官方全量 flags 手册；不要自行套用 Claude / OpenCode 其它参数。
- **版本/限制：** 此份为官方仓库明确展示的常用命令清单，不宣称穷尽全部 flags。

## 8. 本机核对命令

```bash
mcode --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
