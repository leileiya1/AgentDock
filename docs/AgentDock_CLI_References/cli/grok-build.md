# Grok Build · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`grok`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://docs.x.ai/build/cli/reference
- **Settings Reference：** https://docs.x.ai/build/settings/reference
- **Headless：** https://docs.x.ai/build/cli/headless-scripting
- **Sessions：** https://docs.x.ai/build/sessions

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
grok
grok login
grok logout
grok inspect
grok models
grok mcp list
grok plugin list
grok sessions list
grok export SESSION_ID
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 工作目录 | `--cwd <path>` | 自定义工作根目录 |
| 会话 | `-r, --resume [id]` | 恢复指定或最近会话 |
| 会话 | `-c, --continue` | 继续当前工作目录最近会话 |
| 会话 | `-s, --session-id <UUID>` | 指定新会话标识 |
| 会话 | `--fork-session` | 恢复时分叉出新会话 |
| 隔离 | `-w, --worktree [name]` | 在 Git Worktree 中执行 |
| 隔离 | `--ref <ref>` | worktree 的起始 git ref |
| 验证 | `grok inspect --json` | 输出本目录可发现配置 |
| 验证 | `grok models` | 列出模型 |
| 认证 | `grok login --device-auth` | 无头环境的设备码登录 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
grok --cwd ./project
grok --resume SESSION_ID
grok --worktree feature-test
grok inspect --json
grok models
```

## 5. 配置文件与生效范围

- Settings Reference 管理规则、权限、沙箱等默认项。
- `grok inspect --json` 很适合 AgentDock 检查配置发现状态。

## 6. AgentDock 集成建议

**推荐接入方式：** 先检测 grok inspect --json，再按 session/worktree 管理任务。

**功能标签：** 交互 / 恢复 / worktree / MCP / 插件 / 检查配置。

统一适配器至少保存以下字段：

```yaml
adapter_id: grok-build
executable: "grok"
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

- 官方 Reference 声明仅列出常用标志，不是完整清单；务必调用 `grok --help` 与子命令 --help。
- 提供 sessions / plugins / MCP / import、export 等独立子命令。
- 若采用 headless 执行，需额外核对 Headless 专页里的输出结构与权限策略。

## 8. 本机核对命令

```bash
grok --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
