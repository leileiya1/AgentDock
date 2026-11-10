# Aider · CLI 参数操作手册

> 整理日期：2026-11-06　|　分类：第三方开源　|　实际命令：`aider`

> 本文是**中文参数速查与集成笔记**，不是官方原文的完整复制。只对官方可见文档中确认的参数做说明。新版本/平台/账号限制请以本机 `--help` 和官方在线文档为准。

## 1. 官方资料

- **核心参考：** https://aider.chat/docs/config/options
- **配置文件：** https://aider.chat/docs/config/aider_conf.html
- **使用指南：** https://aider.chat/docs/usage.html

## 2. 命令与子命令

以下为常用启动或查询入口（有的项目是独立命令，有的属于 TUI 中的斜杠命令，参见备注）：

```bash
aider
aider --help
aider --model MODEL
```

## 3. 已核对的关键参数

**核对来源：** 官方参数参考页。

| 分类 | 参数 / 命令 | 用途、行为与注意点 |
|---|---|---|
| 模型 | `--model <name>` | 选择模型 |
| 模型 | `--reasoning-effort <value>` | 设置 API 推理强度（取决于模型） |
| 模型 | `--thinking-tokens <n>` | 设置 thinking token 上限（依模型） |
| 调用 | `--timeout <seconds>` | API 请求超时 |
| 编辑 | `--edit-format <name>` | 编辑补丁格式 |
| 编辑 | `--architect` | 以 architect 模式协同修改 |
| 编辑 | `--auto-accept-architect` | 自动接受 architect 建议 |
| 任务 | `--message <text>` | 通过命令行提供任务消息 |
| 任务 | `--message-file <path>` | 从文件读取任务消息 |
| 测试 | `--auto-test` | 修改后自动运行测试（结合配置） |
| 测试 | `--test-cmd <cmd>` | 配置测试命令 |
| 安全 | `--verify-ssl / --no-verify-ssl` | 验证或跳过 TLS 证书；不建议关闭 |

## 4. 使用示例

以下仅展示命令组合的用法，模型名、账号、文件路径、功能权限需按环境调整：

```bash
aider --model MODEL --message "改进异常处理"
aider --reasoning-effort medium
aider --test-cmd "pytest -q" --auto-test
```

## 5. 配置文件与生效范围

- 大量选项支持 `AIDER_*` 环境变量以及 `.aider.conf.yml` 持久配置。
- 逐项 Default、环境变量名见官方 Options Reference。

## 6. AgentDock 集成建议

**推荐接入方式：** 按 Git 文件修改工具适配，不要强行作为流式多工具 Agent。

**功能标签：** 交互 / 直接任务 / 模型 / 编辑格式 / Git / 测试。

统一适配器至少保存以下字段：

```yaml
adapter_id: aider
executable: "aider"
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

- Aider 强调 Git-aware 文件编辑，与 Claude Code 的会话事件输出契约不同。
- `--edit-format` 影响模型产生的修改样式，选择不当可能降低补丁成功率。
- 不要为了连接出错随意关闭 SSL 验证。

## 8. 本机核对命令

```bash
aider --help
```

### 待确认项

- 当前安装版本、所在平台、账号/组织策略是否允许上述选项。
- 未在本手册出现的参数不代表产品不支持；**请以官方最新文档和本机帮助为准**。
- 如存在同名但不同语义的参数，必须按各自 CLI 的版本维护映射，不要猜测兼容。
