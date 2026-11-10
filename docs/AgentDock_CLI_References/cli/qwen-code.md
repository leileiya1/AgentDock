# Qwen Code · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`qwen`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://qwenlm.github.io/qwen-code-docs/en/users/configuration/settings/
- **Headless Mode：** https://qwenlm.github.io/qwen-code-docs/en/users/features/headless/
- **Daemon：** https://qwenlm.github.io/qwen-code-docs/en/users/qwen-serve/
- **仓库：** https://github.com/QwenLM/qwen-code

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
qwen
qwen --help
/help
/model
/mcp
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 模型 | `-m, --model <id>` | 指定会话模型 |
| 非交互 | `-p, --prompt <text>` | 从命令行发起无交互任务 |
| 输出 | `-o, --output-format text\|json\|stream-json` | 文本、一次性 JSON、流式 JSON |
| 输入 | `--input-format text\|stream-json` | 输入格式；stream-json 需配套输出参数 |
| 输出 | `--include-partial-messages` | 输出增量事件，需 stream-json |
| 权限 | `--approval-mode <mode>` | plan/default/auto-edit/auto/yolo |
| 权限 | `--yolo` | 自动批准全部工具调用 |
| 沙箱 | `-s, --sandbox` | 使用沙箱 |
| 沙箱 | `--sandbox-image <uri>` | 选择沙箱容器镜像 |
| Prompt | `--system-prompt <text>` | 覆盖系统 Prompt（具体行为见官方文档） |
| Prompt | `--output-style <name>` | 如 Concise / Proactive / Explanatory / Learning |
| 调试 | `-d, --debug` | 调试输出 |
| 帮助 | `-h, --help` | 查看本机 flags |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
qwen -p "只分析，不修改" --approval-mode plan
qwen -p "整理重构建议" --output-format json
qwen -p "审查测试" --output-format stream-json --include-partial-messages
qwen --model MODEL
```

## 5. 配置文件与生效范围

- 配置层级包含用户级/项目级 settings；具体路径以 Settings 文档为准。
- Daemon / ACP / SDK 应优先查专页，不要把流式 stdout 当成唯一接入方案。

## 6. AgentDock 集成建议

**推荐接入方式：** stream-json + stdin 双向协议；Daemon/ACP 更适合多 Agent 调度。

**功能标签：** 交互 / headless / stream-json / 权限 / Agent / Daemon / MCP。

统一适配器至少保存以下字段：

```yaml
adapter_id: qwen-code
executable: "qwen"
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

- `--input-format stream-json` 必须显式启用 `--output-format stream-json`。
- Qwen 的 `auto-edit` 与 Gemini 的 `auto_edit` 拼写不同。
- `--output-style` 遇到替换系统 Prompt 的场景可能不生效。

## 8. 本机核对命令

```bash
qwen --help
qwen --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
