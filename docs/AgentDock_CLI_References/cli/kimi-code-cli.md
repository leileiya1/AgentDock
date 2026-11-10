# Kimi Code CLI · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`kimi`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://www.kimi.com/code/docs/en/kimi-code-cli/reference/kimi-command
- **ACP 子命令：** https://www.kimi.com/code/docs/en/kimi-code-cli/reference/kimi-acp.html
- **配置覆写：** https://www.kimi.com/code/docs/en/kimi-code-cli/configuration/overrides
- **环境变量：** https://www.kimi.com/code/docs/en/kimi-code-cli/configuration/environment-variables

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
kimi
kimi provider
kimi vis
kimi acp
kimi --help
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 版本 | `-V, --version` | 显示版本 |
| 会话 | `-S, --session [id]` | 恢复具体会话，省略 ID 使用选择器 |
| 会话 | `-c, --continue` | 继续当前目录最近会话 |
| 模型 | `-m, --model <alias>` | 本次运行选择模型别名 |
| 非交互 | `-p, --prompt <text>` | 运行一次提示并将输出写入 stdout |
| 输出 | `--output-format text\|stream-json` | 仅在 --prompt 时使用 |
| 权限 | `-y, --yolo` | 尽量自动执行日常操作，风险操作仍可能询问 |
| 权限 | `--auto` | Never Ask 模式，除计划审阅外更少交互 |
| 模式 | `--plan` | 从 Plan 模式开始 |
| 技能 | `--skills-dir <dir>` | 自定义 Skills 目录，可重复 |
| Agent | `--agent <name>` | 使用指定主 Agent；不能与 session/continue 同用 |
| 帮助 | `-h, --help` | 参数帮助 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
kimi --model MODEL --plan
kimi -p "检查最近的提交" --output-format stream-json
kimi -c
kimi -S SESSION_ID
kimi --skills-dir ./skills
```

## 5. 配置文件与生效范围

- 模型别名来自 default_model / 配置文件。
- 自定义 Provider、Hooks、Sub-Agents 另见官方配置与扩展章节。

## 6. AgentDock 集成建议

**推荐接入方式：** -p stream-json 用于无交互；长会话用 session ID / ACP。

**功能标签：** 交互 / 单任务 / 流式 / 会话恢复 / 权限 / 技能 / ACP。

统一适配器至少保存以下字段：

```yaml
adapter_id: kimi-code-cli
executable: "kimi"
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

- `--yolo` 在 Kimi 不等同于“完全无询问”，其文档描述为 Ask When Needed。
- `--auto` 才对应 Never Ask，但应先在受限仓库中验证。
- Kimi 旧版 Python kimi-cli 与新的 Kimi Code CLI 并不完全兼容。

## 8. 本机核对命令

```bash
kimi --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
