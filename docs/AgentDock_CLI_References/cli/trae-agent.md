# Trae Agent · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商开源　|　实际命令：`trae-cli`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://github.com/bytedance/trae-agent#-usage
- **工具列表：** https://github.com/bytedance/trae-agent/blob/main/docs/tools.md
- **执行轨迹：** https://github.com/bytedance/trae-agent/blob/main/docs/TRAJECTORY_RECORDING.md

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
trae-cli run
trae-cli interactive
trae-cli show-config
trae-cli run --help
trae-cli interactive --help
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 命令 | `run <prompt>` | 执行一次软件开发任务 |
| 命令 | `interactive` | 交互任务会话 |
| 命令 | `show-config` | 展示当前配置 |
| 模型 | `--provider <name>` | 设置 Provider，如 openai/anthropic/ollama |
| 模型 | `--model <name>` | 指定模型 |
| 执行 | `--max-steps <n>` | 限制 Agent 最大步骤数 |
| 执行 | `--working-dir <path>` | 任务执行目录 |
| 轨迹 | `--trajectory-file <path>` | 将执行过程保存为轨迹 JSON |
| 输出 | `--must-patch` | 要求输出/生成补丁 |
| 容器 | `--docker-image <image>` | 使用容器镜像执行 |
| 容器 | `--docker-container-id <id>` | 使用已有容器；不与 working-dir 并用 |
| 容器 | `--dockerfile-path <file>` | 从 Dockerfile 构造运行环境 |
| 容器 | `--docker-image-file <tar>` | 加载本地镜像 tar |
| 容器 | `--docker-keep <value>` | 是否保留容器 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
trae-cli run "修复失败测试" --provider openai --model MODEL
trae-cli run "补齐测试" --working-dir ./repo --trajectory-file ./trace.json
trae-cli interactive --max-steps 30
trae-cli run "分析依赖" --docker-image python:3.12
```

## 5. 配置文件与生效范围

- 可先使用 `trae-cli show-config` 检查 provider、model、权限配置。
- 轨迹日志很适合作为 AgentDock 统一审计的输入，但注意脱敏。

## 6. AgentDock 集成建议

**推荐接入方式：** 优先 run + 轨迹文件，Docker 适合隔离执行。

**功能标签：** run / 交互 / Provider / Docker / 轨迹 / 工具。

统一适配器至少保存以下字段：

```yaml
adapter_id: trae-agent
executable: "trae-cli"
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

- Docker 容器内命令会影响文件系统；管理好挂载目录和容器生命周期。
- `--docker-container-id` 与 `--working-dir` 的组合限制见官方说明。
- Trae Agent 开源框架与 TRAE IDE 产品不是同一个 CLI 命令。

## 8. 本机核对命令

```bash
trae-cli --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
