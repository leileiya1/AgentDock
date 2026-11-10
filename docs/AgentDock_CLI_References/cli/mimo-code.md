# MiMo Code · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`mimo`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://github.com/XiaomiMiMo/MiMo-Code
- **运行配置说明：** https://github.com/XiaomiMiMo/MiMo-Code#configuration
- **项目官网：** https://mimo.xiaomi.com

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
mimo
mimo run
mimo models
mimo --help
mimo run --help
```

## 3. 已核对的关键参数

**核对来源：** 官方 README（仅部分 CLI 标志）。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 运行 | `mimo` | 启动 TUI |
| 运行 | `mimo run "prompt"` | 无交互运行任务 |
| 模型 | `mimo models` | 查看可用模型 |
| 权限 | `--dangerously-skip-permissions` | 跳过权限提示；仅隔离环境 |
| 环境 | `MIMOCODE_DANGEROUSLY_SKIP_PERMISSIONS=1` | 通过环境变量开启高风险跳过 |
| 配置 | `MIMOCODE_HOME` | 覆盖数据/配置位置（依平台） |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
mimo run "审查模块设计"
mimo models
mimo run --dangerously-skip-permissions "仅在隔离容器中执行"
```

## 5. 配置文件与生效范围

- 用户级 `~/.config/mimocode/mimocode.jsonc` 或项目级 `.mimocode/mimocode.jsonc`。
- 不要把 API Key 提交进 Git；本地配置中可能明文保存。

## 6. AgentDock 集成建议

**推荐接入方式：** 只启用已确认 run/模型/权限参数，后续通过 --help 自动扩展。

**功能标签：** 交互 / headless / 模型查询 / 配置 / 权限。

统一适配器至少保存以下字段：

```yaml
adapter_id: mimo-code
executable: "mimo"
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

- 官方仓库明确确认危险权限参数和 headless `run`；其它模型、输出参数需要从本机帮助或源码确认。
- MiMo Code 参考 OpenCode，但不能据此默认继承所有 OpenCode CLI 参数。
- 官方配置文件支持自定义兼容 OpenAI 的 API endpoint。
- **版本/限制：** 缺少独立且完整的官方 CLI flags 页；本手册仅列已在 README 中确认的项。

## 8. 本机核对命令

```bash
mimo --help
mimo run --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
