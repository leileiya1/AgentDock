# AgentDock CLI 适配器设计手册

> 这是一份原创工程设计建议，不是任何厂商官方 API 契约。

## 核心结构

```text
AgentDock Controller
  ├─ Registry（安装路径、CLI 版本、能力探测）
  ├─ Scheduler（队列、依赖、取消、重试）
  ├─ Session Store（厂商 + 工作区 + 会话 ID）
  ├─ Auth/Permissions（用户确认、最小授权、组织策略）
  ├─ Workspace Manager（Git Worktree、锁与清理）
  └─ Adapter Host
      ├─ CLI Stream Adapter (stdout/stderr/JSONL)
      ├─ ACP Adapter (stdio / transport specific)
      ├─ App-Server / HTTP Adapter
      └─ PTY Adapter (交互式终端 fallback)
```

## Adapter 应保存的最小数据

| 字段 | 作用 |
|---|---|
| `id` | 厂商特定的适配器主键 |
| `command` | 可执行文件绝对路径 |
| `version` | 执行 `--version` 的结果 |
| `capabilities` | 探测后持久化的能力集 |
| `workspace_id` | 项目/worktree 唯一 ID |
| `session_id` | 厂商内部会话标识 |
| `approval_mode` | 厂商原生审批策略 |
| `output_protocol` | plain/json/jsonl/acp/app-server |
| `stop_strategy` | cancel API、SIGINT、强杀与兜底策略 |
| `last_event_seq` | 去重和事件顺序辅助字段 |

## 统一事件建议

```json
{"kind":"tool_start","agent":"claude-01","session":"provider-session","tool_id":"xxx","tool":"Bash","ts":"..."}
{"kind":"approval_request","agent":"codex-02","request_id":"yyy","operation":"write","risk":"medium"}
{"kind":"final_result","agent":"qwen-03","text":"...","exit_code":0}
```

以上 schema 为 AgentDock 内部设计示意。请在 Adapter 中把各厂商原始事件转成此格式，保留 `raw_event` 以支持排障。

## 多 Agent 协作

1. 为每个并行写操作分配独立 Git worktree，禁止多个 Agent 在同一工作树里不加锁写入。
2. 任务传递用明确的交接包：目标、输入文件快照、禁止操作、结果、commit、测试日志。
3. 只有 Controller 能触发跨 Agent 任务；Agent 自行请求的子任务也应进入统一任务队列。
4. 先用 patch/commit 作结果交换，再按 review / merge 策略落到共享分支。
5. CLI 的权限绕过参数不是默认设置，应向用户显示风险和有效工作区。

## 容错与安全

- 子进程 stdout/stderr 分开接收；行级缓冲和 chunk 模式都要支持。
- JSONL 解析遇到无法识别类型的事件时存为原始事件，不要丢弃。
- 退出码为 0 也需要检查是否存在 `final_result`；失败或中断要保留状态。
- 设置任务截止时间和心跳；长时间无输出不等价于 CLI 已死。
- 敏感信息（API Key、token、用户路径）在日志与会话导出时脱敏。
- 外网服务器绑定必须默认本地回环并配鉴权；不要默认自动授权所有工具。
- 命令行转义必须使用 argv 数组，而不是拼接 shell 字符串。

## 如何维护参数库

不要完全依赖网页抓取；随版本更新时：检查官方 CLI Reference、在沙箱里执行 `--help`、保存版本、做参数兼容测试；最终由人工/CI 审核更新 Markdown 与 Adapter。
