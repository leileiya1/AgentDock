# Mistral Vibe · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`vibe`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://docs.mistral.ai/vibe/code/cli/work-with-cli
- **配置参考：** https://docs.mistral.ai/vibe/code/cli/configuration-reference
- **安装使用：** https://docs.mistral.ai/vibe/code/cli/install-setup

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
vibe
vibe --prompt "..."
vibe --help
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 非交互 | `-p, --prompt <text>` | 执行单次任务 |
| 非交互 | `--prompt-file <file>` | 从文件读取任务提示 |
| 模型 | `--model <alias>` | 本次任务覆盖 active_model |
| Prompt | `--system-prompt-file <path>` | 使用文件替代默认系统指令 |
| 上限 | `--max-turns <n>` | 控制最多 Agent 轮次 |
| 上限 | `--max-tokens <n>` | 限制总输入与输出 token |
| 上限 | `--time-limit <seconds>` | 运行时长上限 |
| 工具 | `--enabled-tools <pattern>` | 只启用匹配工具，可重复 |
| 工具 | `--disabled-tools <pattern>` | 排除匹配工具，可重复 |
| 审批 | `--auto-approve` | 批准全部工具调用，可能受管理员策略限制 |
| 审批 | `--yolo` | --auto-approve 别名 |
| 输出 | `--output text\|json\|streaming` | 纯文本、结束 JSON、流式结构化消息 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
vibe -p "给出代码审查建议" --max-turns 5 --output json
vibe --prompt-file ./task.md --time-limit 300
vibe -p "修改测试" --enabled-tools bash --output streaming
```

## 5. 配置文件与生效范围

- 配置文件定义 default_agent、active_model 和工具许可清单。
- 项目级 `AGENTS.md` 仍可能影响任务行为，即使使用自定义系统 Prompt。

## 6. AgentDock 集成建议

**推荐接入方式：** -p 程序模式适合有超时和轮次上限的批任务。

**功能标签：** 交互 / 程序模式 / 流事件 / 输出 / 预算限制 / 工具过滤。

统一适配器至少保存以下字段：

```yaml
adapter_id: mistral-vibe
executable: "vibe"
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

- 程序模式无法回答审批提示；未经许可的工具会被拒绝。
- `--auto-approve` 可能被组织策略禁用；不能把审批全开当成可靠自动化。
- JSON 与 streaming 的事件输出结构不同，需分别适配。
- **版本/限制：** 流式格式名称是 `streaming`，不要与 Claude 的 `stream-json` 混淆。

## 8. 本机核对命令

```bash
vibe --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
