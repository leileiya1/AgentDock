# AgentFlow Provider Protocol v1.1

> 状态：v1.1 已实现；核心 crate：`crates/provider-protocol`；传输：本地进程 stdio NDJSON JSON-RPC 2.0。

## 1. 目标与边界

协议固定的是“AgentFlow 核心 ↔ Provider 适配器”的边界。Claude、Codex 或其他 CLI/API 改参数时，只需更新对应 Provider sidecar，不需要修改编排器、数据库或桌面端。

协议不能让未知的上游变化凭空自愈：仍需有人发布兼容新版 CLI/API 的 sidecar，但升级范围被限制在独立 Provider 包中。内置 Provider 保留为默认实现；外部包使用相同 ID 时优先，因而可作为紧急兼容补丁。

## 2. Provider 包

应用启动时扫描 `<app_data>/providers/<package>/provider.json`：

```json
{
  "manifestVersion": 1,
  "id": "acme_agent",
  "displayName": "Acme Agent",
  "protocolVersion": "1.0",
  "executable": "bin/provider",
  "args": [],
  "transport": "stdio-json-rpc",
  "capabilities": {
    "development": true,
    "review": true,
    "streaming": true,
    "structuredOutput": true,
    "sandbox": true,
    "resume": false
  },
  "executionLocation": "local",
  "dataEgress": "none",
  "permissions": {
    "worktreeRead": true,
    "worktreeWrite": true,
    "networkDomains": [],
    "commands": []
  },
  "security": {
    "publisher": "acme",
    "artifactSha256": "<64 位十六进制>",
    "signature": "<base64 Ed25519 签名>"
  },
  "enabled": true
}
```

- ID 为 2–64 字符，首字符小写，后续只允许小写字母、数字、`-_.`。
- executable 必须是包目录内的相对路径；绝对路径和 `..` 会被拒绝。
- 单个坏包只记入注册问题，不阻止 daemon 和其他 Provider 启动。
- manifest/协议主版本不兼容时拒绝加载；次版本允许向后兼容扩展。
- v1.1 外部包必须声明执行位置、数据外发级别和最小权限；未声明这些字段的 v1.0 包会显示为“已隔离”，不会执行。
- `security.artifactSha256` 必须与 executable 内容一致；签名覆盖 Provider ID、协议版本、制品摘要和权限摘要。
- `<app_data>/providers/provider-trust.json` 是本机信任根，只保存 publisher → Ed25519 公钥。公钥未固定、签名无效、制品被替换或权限变化时，Provider 都进入隔离状态，需要重新信任/安装，不能静默放行。

## 3. 进程与消息

核心为每次探测或 run 启动一个 sidecar。stdin/stdout 每行一个 JSON-RPC 消息；stdout 不得混入普通日志，诊断写 stderr。

核心依次调用：

1. `handshake`：协商协议版本并校验 Provider ID。
2. `health`：返回 `ready | degraded | unavailable`，仅用于探测。
3. `run`：传入独立的 RPC request ID，以及权威 task ID、revision、评审 commit SHA、角色、worktree、run 目录、输入文件、权限、双超时和可选 `resumeSessionId`。Provider 必须直接回显任务身份，不应从提示词解析。
4. `shutdown`：请求 sidecar 正常退出；超时后由核心终止。

run 期间 sidecar 可发送 `event` notification，参数必须是共享 `AgentEvent`。run 响应为带标签的 `development` 或 `review` 结构化结果，并可返回 `sessionId`、`costUsd`、`tokensIn`、`tokensOut`；核心仍会用正式 JSON Schema 二次校验，协议结果不是免检事实。

`resumeSessionId` 是 Provider 自己生成的不透明值，仅当用户开启会话复用且该 Provider 声明 `resume=true` 时提供。核心只会复用同一任务、同一角色、同一 Provider 的历史会话；它不能替代 AgentFlow 保存的提交、测试、审查结果和跨轮摘要。

## 4. 能力与动态 UI

`providerList` 返回 ID、显示名、来源、协议版本、能力、执行位置、外发级别、权限、信任状态、可用性和问题。桌面端只按能力展示开发/审查候选，因此安装新 Provider 不需要重新生成枚举或改 UI。

`AgentKind` 的线格式为开放字符串；Rust 对七个内置 ID 保留已知变体，对其他合法 ID 使用 `External(String)`。SQLite 原字段无需迁移。

## 5. 安全与故障语义

- 继承环境前先应用项目 env denylist；密钥不得出现在 manifest、RPC、日志或数据库。
- 绝对超时、空闲超时、取消都会终止 sidecar；stderr 最多保留 1 MiB 并标记截断。
- 开发/审查角色必须匹配 manifest capability；移除已被任务引用的包时返回明确的未安装错误。
- 外部包等同本地可执行代码，只允许已固定 publisher 公钥、签名与 SHA-256 都通过的包进入可执行注册表。其他包保留诊断信息但隔离。
- `dataEgress != none` 或 `executionLocation != local` 时，任务创建必须获得显式外发同意；同意覆盖所选委员会成员和降级链，Provider 权限变化后旧同意失效。

## 6. 一致性测试

`conformance_provider` 是最小参考实现；`crates/provider-protocol/tests/conformance.rs` 验证发现、握手、健康检查、事件、结构化结果及路径逃逸隔离。第三方 Provider 应先复制其交互模型并通过同等测试。
