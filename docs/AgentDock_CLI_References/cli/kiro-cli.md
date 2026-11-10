# Kiro CLI · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`kiro-cli`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://kiro.dev/docs/cli/reference/cli-commands/
- **CLI Setup：** https://kiro.dev/docs/cli/setup/
- **文档首页：** https://kiro.dev/docs/cli/

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
kiro-cli
kiro-cli chat
kiro-cli agent
kiro-cli mcp list
kiro-cli acp
kiro-cli crew
kiro-cli --help-all
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 全局 | `-v, --verbose` | 增加日志级别，可重复 |
| 全局 | `--agent <name>` | 指定自定义 Agent |
| 全局 | `--v3` | 选择 V3 Agent harness |
| 全局 | `--v2` | 选择 V2 Agent harness；与 V3 互斥 |
| 全局 | `--help-all` | 列出所有子命令帮助 |
| chat | `--no-interactive` | 单次无交互输出 |
| chat | `-r, --resume` | 恢复目录最近会话 |
| chat | `--resume-picker` | 使用选择器查找会话 |
| chat | `--resume-id <id>` | 恢复指定会话 |
| chat | `--list-sessions` | 列出保存的会话 |
| chat | `--list-models` | 查询可用模型 |
| chat | `--model <id>` | 选择模型 |
| chat | `--effort <level>` | 低到高的推理级别，具体取值见官方 |
| chat | `--trust-tools <list>` | 信任特定工具，V3 交互与非交互行为可能不同 |
| chat | `--trust-all-tools` | 无需逐次确认工具，高风险 |
| chat | `--sandbox` | V3 headless 时启用本地命令沙箱 |
| chat | `--require-mcp-startup` | MCP 未启动时以错误码退出 |
| acp | `kiro-cli acp` | 通过 stdin/stdout 运行 ACP 服务器（隐藏子命令） |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
kiro-cli chat --no-interactive "分析项目结构"
kiro-cli chat --resume-id SESSION_ID
kiro-cli chat --effort high
kiro-cli chat --list-models --format json
kiro-cli acp
```

## 5. 配置文件与生效范围

- 配置和 Agent 设置依 Kiro CLI 版本；应读取 `kiro-cli --help-all`。
- `--require-mcp-startup` 适合对 MCP 有硬依赖的 AgentDock 任务。

## 6. AgentDock 集成建议

**推荐接入方式：** 服务化可用 ACP；会话须记录 V2/V3 harness 与权限策略。

**功能标签：** 交互 / 无交互 / 恢复 / 模型 / 自定义 Agent / MCP / ACP / 沙箱。

统一适配器至少保存以下字段：

```yaml
adapter_id: kiro-cli
executable: "kiro-cli"
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

- `acp` 是隐藏命令：常规 --help 不展示不代表没有。
- V2 / V3 harness 可能改变权限、工具名称、事件行为；适配器必须记录 harness。
- 不要默认在所有版本启用 `--trust-all-tools`。
- **版本/限制：** V3 的权限规则和 V2 存在差异。

## 8. 本机核对命令

```bash
kiro-cli --help
kiro-cli --help-all
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
