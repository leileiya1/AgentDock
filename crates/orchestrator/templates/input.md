# 任务 TASK-{{TASK_SEQ}} r{{REVISION}}

内部 task_id：`{{TASK_ID}}`。交付 JSON 中的 `task_id` 必须逐字使用此值，`revision` 必须为 `{{REVISION}}`。

## 需求

### {{TITLE}}

{{DESCRIPTION}}

{{GUIDANCE}}

## 项目规则

{{RULES}}

{{HISTORY}}

## 边界

只允许修改当前 worktree。禁止改动 `.agentflow/`、`CLAUDE.md`、`AGENTS.md`、`.claude/` 与 CI 配置，除非需求明确要求。不要执行白名单外命令。

## 交付

完成后，将结果先写入 `.agentflow-out/result.tmp.json`，再原子重命名为 `.agentflow-out/result.json`。必须符合以下 JSON Schema：

```json
{{RESULT_SCHEMA}}
```
