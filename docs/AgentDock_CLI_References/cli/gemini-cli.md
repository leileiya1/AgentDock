# Gemini CLI · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：厂商官方　|　实际命令：`gemini`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/cli-reference.md
- **配置参考：** https://github.com/google-gemini/gemini-cli/blob/main/docs/reference/configuration.md
- **官方仓库：** https://github.com/google-gemini/gemini-cli

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
gemini
gemini --help
/model
/help
/stats
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 通用 | `-m, --model <id>` | 指定模型 |
| 非交互 | `-p, --prompt <text>` | 非交互执行；可从 stdin 读取 |
| 交互 | `-i, --prompt-interactive <text>` | 先执行提示，再留在交互 TUI |
| 隔离 | `-w, --worktree [name]` | 在新的 Git worktree 执行；需启用实验性设置 |
| 安全 | `-s, --sandbox` | 沙箱模式 |
| 安全 | `--approval-mode <mode>` | default / auto_edit / yolo / plan |
| 安全 | `--skip-trust` | 跳过工作目录信任询问 |
| 安全 | `--yolo` | 旧版全自动审批参数，建议改用 approval-mode |
| 工具 | `--allowed-tools <list>` | 指定无需弹窗确认的工具 |
| 工具 | `--allowed-mcp-server-names <list>` | 限制可用 MCP 服务列表 |
| 协议 | `--experimental-acp` | 进入 ACP 模式（旧版文档） |
| 协议 | `--acp` | 新配置文档中使用的 ACP 启动选项；版本差异需检测 |
| 通用 | `-h, --help` | 查看本机 CLI 参数 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
gemini --model MODEL
gemini --prompt "检查这个函数的边界条件"
gemini --approval-mode plan
gemini --sandbox --prompt "分析并给出建议"
```

## 5. 配置文件与生效范围

- 主配置 settings.json；详细键名见官方 Configuration。
- Gemini CLI 与 Antigravity CLI 的历史/迁移关系可能影响账号可用性，但参数并不完全通用。

## 6. AgentDock 集成建议

**推荐接入方式：** 优先 --prompt / ACP，明确 beta/legacy flags 的版本差异。

**功能标签：** 交互 / prompt / 沙箱 / 审批 / worktree / ACP。

统一适配器至少保存以下字段：

```yaml
adapter_id: gemini-cli
executable: "gemini"
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

- Google 同时维护 Antigravity 和 Gemini CLI，不要混用二者选项。
- ACP 的 `--acp` 与 `--experimental-acp` 出现在不同官方参考页，按版本检查。
- 不要默认认为 `--yolo` 在所有部署环境可用。
- **版本/限制：** 部分参数位于旧版仓库文档；若当前命令不识别，请以本机 --help 为准。

## 8. 本机核对命令

```bash
gemini --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
