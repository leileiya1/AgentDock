# 独立代码审查

## 原始需求

### {{TITLE}}

{{DESCRIPTION}}

{{GUIDANCE}}

## 审查对象

- revision commit: `{{COMMIT_SHA}}`
- base commit: `{{BASE_COMMIT}}`
- task_id: `{{TASK_ID}}`
- revision: `{{REVISION}}`
- 统计: {{DIFF_STAT}}

{{FLAGGED_WARNING}}

## diff

{{DIFF}}

## 测试报告

```json
{{TEST_REPORT}}
```

## 输出

最终消息只输出符合下列 Schema 的 JSON，不要添加 Markdown 围栏：

```json
{{REVIEW_SCHEMA}}
```
