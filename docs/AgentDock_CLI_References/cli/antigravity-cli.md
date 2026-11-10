# Antigravity CLI · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`agy`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://antigravity.google/docs/cli/reference/
- **Headless：** https://antigravity.google/docs/cli/headless/
- **ACP：** https://antigravity.google/docs/cli/acp/
- **权限与沙箱：** https://antigravity.google/docs/permissions/

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
agy
agy models
agy agents
/model
/agents
/resume
/plan
/permissions
/tasks
/teamwork
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 执行 | `--mode <mode>` | 设置执行模式：plan、accept-edits、auto（auto 有平台及账户限制） |
| 模型 | `--model <id>` | 为当前会话指定模型 |
| 模型 | `--effort <level>` | 模型允许的推理强度 |
| Agent | `--agent <name>` | 选定自定义 Agent |
| 目录 | `--add-dir <dir>` | 增加工作目录，可重复 |
| 会话 | `-c, --continue` | 继续最近对话 |
| 会话 | `--conversation <id>` | 恢复指定会话 |
| 交互 | `-i, --prompt-interactive <text>` | 执行初始任务并继续交互 |
| 非交互 | `-p, --print, --prompt <text>` | 执行任务并退出 |
| 非交互 | `--output-format text\|json\|stream-json` | 输出格式 |
| 非交互 | `--input-format text\|stream-json` | 输入格式 |
| 非交互 | `--json-schema <schema\|path>` | 强制结构化输出（print 模式） |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
agy --model MODEL --effort high
agy -p "分析代码仓库" --output-format stream-json
agy --continue
agy --conversation SESSION_ID
agy --mode plan
```

## 5. 配置文件与生效范围

- 参考官方 CLI Settings 页面；`agy models` 提供可选模型。
- 以当前版本 `/config` 与 `agy --help` 双重核对默认值。

## 6. AgentDock 集成建议

**推荐接入方式：** 优先 headless 与 ACP，记录模式、账户可用性和推理强度。

**功能标签：** 交互 / headless / JSON / 流事件 / 会话恢复 / 自定义 Agent / ACP。

统一适配器至少保存以下字段：

```yaml
adapter_id: antigravity-cli
executable: "agy"
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

- CLI flag 与 TUI 斜杠命令必须区分，`/plan` 仅在交互界面使用。
- `auto` 模式依平台版本和账户权限，不能在 UI 中始终展示为可用。
- AgentDock 集成建议优先验证 headless 与 ACP 的具体进程协议。
- **版本/限制：** 不同部署平台与账户类型会影响 `auto` 执行模式。

## 8. 本机核对命令

```bash
agy --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
