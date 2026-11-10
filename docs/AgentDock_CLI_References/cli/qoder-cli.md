# Qoder CLI · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`qoder`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://docs.qoder.com/zh/cli/cli-reference
- **配置项与环境变量：** https://docs.qoder.com/zh/cli/settings-reference
- **权限说明：** https://docs.qoder.com/zh/cli/permissions
- **ACP：** https://docs.qoder.com/zh/cli/acp

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
qoder
qoder --help
qoder --list-models
/help
/agents
/permissions
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 工作目录 | `-w, --cwd <path>` | 指定工作区 |
| 工作目录 | `--config-dir <dir>` | 覆盖用户配置根目录 |
| 工作目录 | `--add-dir <dir>` | 附加信任目录；可多次使用 |
| 工作目录 | `--attachment <path>` | 附加文件 |
| 工作目录 | `--worktree [name]` | 在单独 Git worktree 执行 |
| 模型 | `-m, --model <id>` | 模型选择：如 auto、lite、performance 或 BYOK |
| 模型 | `--reasoning-effort <level>` | 覆盖推理强度 |
| 模型 | `--context-window <n>` | 覆盖上下文大小 |
| 模型 | `--list-models` | 输出可用模型列表 |
| 风格 | `--output-style <name>` | 指定输出风格 |
| 日志 | `-d, --debug` | 调试输出 |
| 通用 | `-h, --help` | 当前版本帮助 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
qoder --cwd ./project --model auto
qoder --reasoning-effort high
qoder --worktree feature-1
qoder --attachment ./requirements.md "评审需求"
```

## 5. 配置文件与生效范围

- 默认 `~/.qoder`，可用 `--config-dir` 覆盖。
- 配置作用范围与 CLI 临时覆盖请参看官方配置参考。

## 6. AgentDock 集成建议

**推荐接入方式：** 工作区 + worktree 传参、记录配置目录；ACP 作为进阶接入。

**功能标签：** 交互 / 非交互 / 模型 / worktree / ACP / 附件。

统一适配器至少保存以下字段：

```yaml
adapter_id: qoder-cli
executable: "qoder"
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

- 官方页面还有非交互/审批/协作等分类，建议按功能深入原始手册查看完整参数。
- `--worktree` 会改变文件位置，AgentDock 必须保存工作分支与 checkout 位置。

## 8. 本机核对命令

```bash
qoder --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
