# AgentDock 前端实时操作问题清单（2026-07-22）

## 测试环境

- 安装版：`/Applications/AgentFlow.app`
- 版本：`0.1.0`
- 安装包：`apps/desktop/src-tauri/target/release/bundle/dmg/AgentFlow_0.1.0_aarch64.dmg`
- 测试项目：`/Users/sapiece/innovationProject/test`
- 实时任务：`TASK-001`，实现带持久化失败保护的计数器库
- 开发 Agent：Claude Code
- 审查 Agent：Codex
- 计划审批：开启

## 问题清单

### P0-01 环境显示“就绪”，但已连接 Provider 实际无法运行

**现象**

- 首次引导和设置页显示 Claude Code、Codex“已连接”，侧栏显示“环境就绪”。
- 创建任务后，Claude 规划立即失败：`Not logged in · Please run /login`。
- Codex 规划随后失败：模型缓存缺少 `supports_reasoning_summaries` 字段，并出现等待子进程退出超时。
- 任务无法生成计划，核心开发闭环无法开始。

**复现步骤**

1. 导入 `test` 项目。
2. 保持默认 Claude Code 开发、Codex 审查。
3. 点击“创建并立即开始”。
4. 打开“日志”并展开执行者列表。
5. 分别查看 Claude Code 和 Codex 的计划运行。

**期望**

- “已连接/环境就绪”必须来自一次可执行的登录与协议探测，而不是只检查命令存在。
- 创建任务前的 Preflight 应阻止启动并给出“重新登录/修复缓存/重新检测”入口。

### P0-02 明知 CLI 未安装，规划降级链仍继续调用

**现象**

- 设置页明确显示 Gemini CLI、Qwen Code“未安装”。
- 规划阶段仍依次执行 Gemini 和 Qwen。
- 两次均以退出码 127、`command not found` 失败。
- 最终失败卡片只突出最后一次 Qwen 失败，掩盖了 Claude 未登录和 Codex 运行时不兼容这两个更有价值的根因。

**期望**

- 降级链启动前过滤未安装、未登录、协议不兼容的 Provider。
- 若所有 Provider 均不可用，应在 Preflight 一次性列出原因，不应创建注定失败的运行。
- 最终错误应汇总全部尝试，而不是只显示最后一个 Provider。

### P0-03 macOS 应用签名校验失败

**现象**

- DMG 校验和有效。
- `codesign --verify --deep --strict /Applications/AgentFlow.app` 失败：`code has no resources but signature indicates they must be present`。
- `spctl --assess --type execute /Applications/AgentFlow.app` 同样失败。
- 当前开发机可以直接打开，但在其他 Mac、下载分发或 Gatekeeper 严格环境中可能无法正常安装和启动。

**期望**

- 发布包使用完整的 Developer ID 签名并通过 notarization。
- CI/发布门禁至少执行 `codesign --verify --deep --strict`、`spctl --assess` 和 DMG 校验。

### P1-01 失败后的“补充指引继续”不能解决实际故障

**现象**

- 失败卡片仅提供“补充指引继续”和“取消任务”。
- “补充指引继续”对话框只有文本说明输入框。
- 当前失败是未登录、缓存协议不兼容和 CLI 未安装，补充任务文字无法修复。

**期望**

- 根据失败类型提供“重新认证”“重新检测”“安装 CLI”“从可用 Provider 重试”“编辑降级链”等操作。
- 修复环境后允许从同一检查点继续，不要求重新创建任务。

### P1-02 环境检测页面信息不完整

**现象**

- 实际页面主要展示 Git 和 Provider 状态。
- 未明确展示操作系统、架构、AgentFlow 版本、Bun/Node、Shell、网络、磁盘空间、项目目录权限、系统钥匙串、CLI 版本等结果。
- 在检测信息不足的情况下仍显示“环境就绪”。

**期望**

- 按核心稳定性清单逐项展示检测状态、版本和失败原因。
- “环境就绪”应区分“可打开应用”和“至少有一套可完成计划→开发→审查的组合”。

### P1-03 系统时间线没有记录关键降级与失败事件

**现象**

- `TASK-001` 已连续尝试四个规划 Provider。
- “系统详情 1 条”中只有“你启动了任务”。
- Provider 降级、失败原因和进入人工介入状态没有出现在系统详情时间线。

**期望**

- 每次 Provider 启动、失败、降级、最终阻断都应进入事件时间线。
- 时间线应能直接定位对应执行者日志。

### P1-04 日志页默认展开技术详情

**现象**

- 首次进入日志页即可直接看到 Codex 缓存字段错误、超时和原始协议内容。
- 用户可读总结不足时，界面回退为复杂技术日志。

**期望**

- 默认展示可读的失败摘要和下一步操作。
- 原始日志保持折叠，仅在“查看技术详情”后展开。

### P1-05 项目存在失效 worktree 注册，但导入和 Preflight 未提示

**现象**

- `git worktree list --porcelain` 显示多个 `prunable gitdir file points to non-existent location` 的旧 AgentFlow worktree。
- 项目导入成功，界面显示“标准本地仓库”，没有陈旧工作树诊断或清理建议。

**期望**

- 导入或 Preflight 检测 AgentFlow 自己遗留的失效 worktree。
- 提供只清理失效注册的安全入口，并明确不会删除项目源文件。

### P2-01 最大返工轮数非法输入反馈异常

**现象**

- 在“最大返工轮数”中全选后输入 `0`，字段静默变为 `1`。
- 全选后输入 `-1`，字段会变为 `11`。
- 页面没有解释允许范围或显示校验错误。

**期望**

- 允许用户完成输入后再校验。
- 非法值应显示明确范围提示，不应在输入过程中变成另一个有效数字。

### P2-02 新建任务缺少独立的结构化验收条件

**现象**

- 新建任务只有标题和描述，验收条件只能混写在描述中。
- 无法逐条标记构建、测试、行为和人工验收要求。

**期望**

- 提供可增删的验收条件列表，并在验证、审查和最终批准页面逐条回显结果。

## 已通过项目

- Bun、TypeScript、Vite、Rust/Tauri 生产构建成功。
- DMG 生成成功且 `hdiutil verify` 校验通过。
- 安装版与最新构建产物的主程序 SHA-256 一致。
- 安装版可以从 `/Applications/AgentFlow.app` 启动。
- 首次引导可以导入 `/Users/sapiece/innovationProject/test`。
- 新建任务按钮与 `Command+N` 快捷键可用。
- 任务页数字键 `1`～`5` 可以切换标签页。
- 侧栏折叠/展开正常。
- Diff 和审查在没有 revision 时有空状态。
- 失败后原始项目保持干净，工作内容没有写入主工作区。
- 任务失败后安全停在人工介入检查点。
- 关闭并重新打开应用后，项目、任务和“需要你处理”状态均能恢复。
- 审计导出对话框可以打开，并明确 JSONL、脱敏和导出范围。

## 当前保留状态

- `TASK-001` 保留在“需要你处理”，便于继续查看四个 Provider 的失败日志。
- 未取消或删除任务。
- 未修改 `/Users/sapiece/innovationProject/test` 主工作区文件。
- 未修改 AgentDock 源码来规避上述问题。

## P0 修复复测（2026-07-22 15:40）

### 复测环境

- 从当前工作区重新运行 `bun run app:build`，TypeScript、Vite、Rust/Tauri 构建成功。
- 最新安装版已覆盖安装到 `/Applications/AgentFlow.app`；旧安装版保留为 `/Applications/AgentFlow.app.pre-p0-retest`。
- 最新主程序 SHA-256：`ea9cb47fc2141e10850d9cbf9423578e650fa0cb69972b555195cec97c0bada6`。
- 新建真实任务：`TASK-002`（`P0 Provider 预检与降级链复测`）。
- 开发首选：Codex；审查：Claude Code；计划审批保持开启，因此失败前没有修改项目文件。

### P0-01 复测：未通过，Preflight 与真实运行仍不一致

**已经改善**

- 创建并立即开始前会显示“检测环境…”，说明真实 Preflight 接线已经生效。
- 设置页和下拉框能够把 Gemini、Qwen 标为“未安装/不可用”。
- 当前终端侧 `claude auth status --json` 返回已登录，`codex login status` 返回已登录。

**仍然失败**

- Preflight 判定任务可运行并创建 `TASK-002`，侧栏继续显示“环境就绪”。
- Codex 真实规划仍失败：模型缓存缺少 `supports_reasoning_summaries`，刷新模型时等待子进程退出超时。
- 降级到 Claude 后，真实规划仍返回 `Not logged in · Please run /login`。
- 两个真实可执行 Provider 均失败，任务在 11 秒后进入“需要你处理”，核心闭环仍未开始。

**结论**

- 当前探测只证明登录状态命令成功，不能证明 Provider 在 AgentFlow 实际沙箱、环境变量和运行参数下可执行。
- Preflight 必须使用与真实 Agent 启动相同的可执行路径、环境清理、沙箱和最小协议探针；至少要能捕获 Codex 模型缓存兼容性和 Claude 在实际运行上下文中的认证不可用。

### P0-02 复测：后端通过，前端汇总仍不完整

**已经通过**

- Provider 下拉框中的 Gemini/Qwen 显示“不可用”且无法选中。
- `TASK-002` 执行者列表只有 Codex、Claude 两项。
- `agent_runs` 中只有两条实际运行记录；没有启动 Gemini 或 Qwen。
- 数据库 `blocked_detail` 正确聚合：Gemini/Qwen 为“未尝试”，Codex/Claude 为退出码 1。

**仍需修复**

- 前端失败卡片只显示最后一次 Claude 失败，仍使用通用文案“这一轮运行失败了”。
- 前端没有展示数据库已经保存的聚合 `blocked_detail`，用户必须逐个打开执行者日志才能发现 Codex 根因。

**结论**

- “不调用未安装 CLI”已经修好。
- “最终错误一次性汇总全部尝试与跳过原因”尚未在前端完成，P0-02 只能判定为部分通过。

### P0-03 复测：发布门禁通过，当前安装包仍不具备发布资格

**门禁实现通过**

- `cargo test -p agentflow-release-engine`：11/11 通过。
- 专门覆盖 `code has no resources but signature indicates they must be present`、ad-hoc 签名、Gatekeeper 拒绝和 SHA-256 不匹配。
- `scripts/verify-macos-bundle.sh` 能完整执行并以退出码 1 拒绝当前包。
- DMG 的 `hdiutil verify` 和 SHA-256 检查通过；本次 SHA-256 为 `dc0a55c40c2a380fe972b6371bcc492b75469aa782f779a921ded675a7d8005d`。

**当前包仍失败**

- `codesign --verify --deep --strict`：签名与资源清单不一致。
- 签名主体检查：仍是 ad-hoc 临时签名。
- `spctl --assess`：未接受该应用；本次系统返回 `Too many open files`。

**结论**

- 门禁已做到 fail-closed，不会把当前坏包误判成可发布。
- 在配置 Developer ID 和 notarization secrets 前，P0-03 的“发布流程防误发”已完成，但“产出可分发安装包”仍受外部证书阻塞。

### 本轮针对性自动测试

- `cargo test -p agentflow-orchestrator preflight --lib`：5/5 通过。
- 前端 `preflight`、阻断对话框、环境状态点测试：10/10 通过。
- 原始 `/Users/sapiece/innovationProject/test` 与 `TASK-002` 工作树均保持干净。

### 新观察

- 环境/Preflight 探测约持续 18 秒，期间只有统一的“检测环境…”状态，没有逐 Provider 进度。
- Codex 技术日志在 UI 中出现重复的一组原始事件；同一缓存错误和会话事件各显示两次。

## Codex 专项修复与最终复测（2026-07-22 16:17）

### 根本原因与修复

1. 本机同时存在独立 Codex `0.144.6`、损坏的 Homebrew Codex，以及 ChatGPT 内置 Codex `0.145.0-alpha.30`；它们共用 `~/.codex/models_cache.json`。缓存由 `0.145.0` 写入后，旧版 `0.144.6` 读取新格式，触发 `missing field supports_reasoning_summaries`。
2. AgentFlow 原来预检会解析 Codex 实际路径，但启动阶段仍使用原始字符串 `codex`，因此预检与运行可能命中不同版本。
3. 修复后，默认路径会读取缓存的 `client_version`，选择与缓存写入版本兼容且能执行的 Codex；用户显式配置的绝对路径仍保持最高优先级。预检、认证检测和真实启动统一使用同一解析器。
4. 真实复测随后暴露并修复了 Codex 严格结构化输出 schema：所有对象字段都进入 `required`，可选字段以 nullable 类型表达，`schema_version` 同时保留 `type: integer` 与 `const: 1`。非严格 Provider 省略可选字段时，解析器会补 `null`，保持 Claude/Gemini 兼容。

### 验证结果

- 设置页 Codex 详情显示：`codex-cli 0.145.0-alpha.30`，路径 `/Applications/ChatGPT.app/Contents/Resources/codex`，账号登录。
- 修复后的计划 schema 直接调用 Codex 成功，退出码 `0`，返回符合 schema 的三步计划。
- AgentFlow 界面创建 `TASK-005`（`Codex 修复最终验收`）：Codex 规划运行 `SUCCEEDED`、退出码 `0`，约 41 秒后进入 `WAITING_FOR_PLAN_APPROVAL`。
- 界面正确显示“等待你批准编码计划”“任务已暂停，批准后才会开始写代码”；未批准计划，因此没有进入开发，也没有调用 Claude。
- `/Users/sapiece/innovationProject/test` 主工作区 `git status --short` 与 `git diff --check` 均无输出。
- `cargo test -p agentflow-contracts`：2/2 通过；`cargo test -p agentflow-agent-adapters`：25/25 通过；编排层 Preflight 测试：5/5 通过。
- `cargo fmt --all -- --check`、TypeScript 检查、Vite 生产构建与 Tauri `.app`/DMG 打包均通过。
- 最终安装版主程序 SHA-256：`aea7c41823b88fc68ea2057f341756a8406d7181e98f59d1d2f58b1be4d46d55`；守护进程 SHA-256：`8c702873d2e2340247a74b9b5b3039fb2683a635a3f13eab75af0a0b002491b0`；DMG SHA-256：`0f454dc795a16cb3ac52f619b5f7dd57ba57ed64bfdbea868c1e0586c76746a7`。

### 结论

- P0-01 中 Codex 的版本/缓存冲突、预检与启动路径不一致、严格 schema 不兼容均已修复并通过真实 UI 闭环验证。
- Claude Code 的额度/登录问题与 Codex 无关，本次没有尝试消耗 Claude 额度。
- P0-03 的 Developer ID 签名与 notarization 仍需要仓库维护者配置 Apple 证书和凭据；本地代码无法代替这些外部凭据。
- 修复前安装版保留在 `/Applications/AgentFlow.app.pre-codex-fix-20260722-1555`，可恢复。

## Claude 登录环境专项修复（2026-07-22 16:34）

### 根本原因

- 终端中的 Claude Code `2.1.217` 登录正常，账号认证方式为 `claude.ai`。
- AgentFlow 为防止无关密钥泄漏，会在启动 Provider 前清空父进程环境，只恢复必要变量；原白名单保留了 `HOME` 和 `PATH`，但遗漏了 `USER` 与 `LOGNAME`。
- 使用与 AgentFlow 相同的最小环境可稳定复现：不含 `USER/LOGNAME` 时，`claude auth status --json` 返回 `loggedIn: false`；加入这两个变量后立即恢复 `loggedIn: true`。
- 因此此前的“Not logged in”是 AgentFlow 启动环境造成的误报，不是终端登录失效，也不是模型问题。

### 修复与复测

- Provider 最小环境白名单加入 `USER`、`LOGNAME`，仍不继承其他未知变量或密钥。
- Claude 的实际启动路径改为复用与预检相同的 `resolve_cli` 结果，避免预检与运行命中不同 CLI。
- 最小环境真实调用不再报告未登录，而是正确返回当前账户实际限制：`You've hit your session limit · resets 7:10pm (Asia/Taipei)`。
- 重新安装后创建 `TASK-006`：Claude 运行日志明确显示“额度状态：rejected”和 19:10 重置；随后自动降级到 Codex，Codex 成功生成计划，任务进入 `WAITING_FOR_PLAN_APPROVAL`。
- 未批准计划，未进入开发；`/Users/sapiece/innovationProject/test` 保持干净。
- `cargo test -p agentflow-agent-adapters`：26/26 通过；TypeScript、Vite、Rust/Tauri 生产构建与 DMG 打包通过。

### 结论

- Claude 登录误判已修复。
- 当前 Claude 无法执行的唯一已确认原因是五小时会话额度耗尽；这是 Anthropic 账户侧限制，等待 19:10 重置后即可再次使用，无需重新登录。

## Provider 能力探测与稳定版本加固（2026-07-22 17:03）

### 已落地机制

- 新增集中支持矩阵 `config/provider-compatibility.json`：记录每个 CLI 的关键参数、固定回归版本和官方包名；Claude Code 固定基线为 `2.1.217`，Codex 固定基线为 `0.145.0-alpha.30`。
- 环境检查把 CLI 明确分为“已验证”“参数兼容但版本未经回归”“不兼容”“未纳入矩阵”，不再把“命令存在”直接等同于“稳定支持”。
- 创建任务、规划、开发和审查进入 Provider 链前，Claude/Codex 都会运行只读、无工具、临时 Git 仓库中的最小真实请求；探针使用与真实任务相同的最小账号环境和实际解析后的可执行路径。
- 真实探针调度已从 Claude/Codex 两路硬编码改成任意数量 CLI 的通用并发批次。只纳入实际任务降级链中已安装、参数兼容、已认证且在矩阵声明了安全 `runtimeProbe` 的本地 CLI；结果按 Provider、可执行路径、版本和认证方式缓存 10 分钟。认证失败、额度/限流、45 秒无响应、CLI 参数或结构化 schema 漂移都会把该 Provider 从本阶段运行链移除，并安全降级到下一项。
- 新增 `probe-provider-runtime` 诊断二进制，可单独复现本机真实探针；新增固定版本 CI 和每周 latest advisory，最新上游只做协议漂移预警，不会自动进入稳定矩阵或改写用户安装。

### 本机真实结果

- Claude Code：`2.1.217`、固定版本已验证、账号登录正常；真实探针准确分类为“额度或速率限制”，不再误报未登录。
- Codex：实际选择 `/Applications/ChatGPT.app/Contents/Resources/codex`，版本 `0.145.0-alpha.30`；固定版本与真实严格 schema 探针均通过。
- 安装版设置页真实显示 Claude/Codex“已连接 · 已验证”，详情可见固定基线、实际版本、认证方式和最终可执行路径。
- 通过界面创建 `TASK-007`，首选开发 Agent 保持 Claude，任务预检耗时约 30 秒后创建成功；实际规划没有启动 Claude，直接由 Codex 执行并在 58 秒后进入 `WAITING_FOR_PLAN_APPROVAL`。
- 计划结果明确记录“实际使用 Codex、未调用 Claude、未修改文件”；未批准计划，`/Users/sapiece/innovationProject/test` 的 `git status --short` 保持无输出。

### 自动验证与安装物

- `cargo test -p agentflow-agent-adapters`：29/29 通过。
- `cargo test -p agentflow-orchestrator -- --test-threads=1`：100/100 通过，另有 3 项依赖真实 GitHub/SSH 环境的测试按设计忽略；并行运行曾出现 1 项既有可复现测试相互污染，单独与串行复跑均通过。
- Rust Clippy（adapter/orchestrator all-targets，warnings-as-errors）、TypeScript、Vite 生产构建、前端 211 项测试、Tauri `.app` 与 DMG 打包均通过。
- 最终安装时，原始构建副本仍因资源封印损坏无法由 LaunchServices 启动；只对 `/Applications/AgentFlow.app` 本机测试副本重新做了 ad-hoc 签名。`codesign --verify --deep --strict` 随后通过，界面能启动并保留 `TASK-007` 状态；这不等于 Developer ID 签名，也不会改变 DMG 发布门禁结论。
- 本机 ad-hoc 安装版主程序 SHA-256：`fecc2861139e5d977cd632913ae33a200fe7c9bb9e243b7163072350d1af32e8`；守护进程 SHA-256：`d90ce476165b86a12ec6a67c7dffed6cd5a97de0ce368b359e17062b25bdac9e`；未改写的 DMG SHA-256：`f3c6b765f296d5c7419ee931a73abbb098746dc11e9d0e34c59a87511a407b69`。
- 旧安装版已移动到废纸篓中的 `AgentFlow.previous-20260722.app`、`AgentFlow.pre-parallel-probe-20260722.app` 和 `AgentFlow.pre-generic-probes-20260722.app`，均可恢复。

### 仍然存在的外部阻塞

- 新构建的 DMG 内原始应用仍会被发布门禁判为资源封印损坏；本机安装副本虽已重新 ad-hoc 签名并可运行，但 Gatekeeper/正式分发仍不接受。在配置 Developer ID Application 证书和 notarization 凭据前，DMG 不能作为正式对外发布包。

## P1 全量修复与安装版复测（2026-07-22 18:03）

### P1-01 已修复：失败后提供针对根因的恢复闭环

- 失败处理卡片现提供“认证 / 安装 Provider”“重新检测”“从可用 Provider 重试”“编辑降级链”“补充指引继续”和“取消任务”，不再只给无法解决认证、额度或兼容性问题的通用重试。
- “从可用 Provider 重试”会先重跑环境与 Provider 探测，再从保存的同一检查点恢复；规划阶段失败会恢复到 `PLANNING`，不会绕过计划审批直接进入开发。
- 安装版在 `TASK-004` 实测：从失败卡片点击重试后，状态先变为“拟定计划”；Codex 成功生成计划后停在“等你批计划”，未进入开发、未写入项目文件。
- 编排层新增 `planner_recovery_resumes_the_same_plan_checkpoint` 回归测试，覆盖 revision 0 的规划恢复语义。

### P1-02 已修复：完整环境信息、并行真实探针与有界降级

- 设置页分开显示“应用基础环境”和“端到端工作流”两级就绪状态，并展示操作系统、架构、AgentFlow 版本、Shell、Node、Bun、DNS、可用磁盘、系统钥匙串和 Git 状态。
- Git、Node、Bun 以及所有已安装 CLI 的无副作用能力检测并行执行；单 CLI 检测最多等待 10 秒，超时会明确显示“不兼容/已停止等待”，不会伪装成未安装。
- 安装版复测时进一步发现同步 `Security.framework` 密钥读取会在 ad-hoc 签名应用中等待 Keychain 授权锁。状态探测现改为只检查 Keychain 条目元数据、不读取密钥内容，并设置 2 秒边界；真正运行 Provider 时仍走原安全密钥读取路径。
- 实机点击“重新检测”后约 12.7 秒收敛，按钮恢复为“重新检测”，没有继续停留在“检测中”。本机显示 macOS 26.5.2、aarch64、AgentFlow 0.1.0、Node v24.18.0、Bun 1.3.14、DNS 可用、磁盘可用 780.4 GB、登录钥匙串可用；应用和端到端工作流均就绪。
- Provider 页面真实显示 Claude Code 与 Codex“已连接 · 已验证”、DeepSeek API“已连接”，未安装或未配置项保持诚实状态。

### P1-03 已修复：Provider 生命周期进入系统时间线并可跳转日志

- 每个新运行会持久化 `provider:started`、`provider:succeeded`、`provider:failed`，载荷包含 task、run、revision、agent、role 和结果状态；规划、开发、审查都使用同一事件链，最终阻断也保留明确事件。
- 安装版 `TASK-004` 真实重试显示“Codex 已开始规划”和“Codex 规划完成”，两条事件均提供“查看对应执行日志”；点击后保持在日志页并定位对应运行。
- 历史运行不会伪造或回填新事件；新事件从本版本产生的新运行开始完整记录。

### P1-04 已修复：技术详情默认折叠

- 新会话的 `technicalLogOpen` 默认值改为 `false`，主日志优先展示可读进展、结果和风险；原始协议/命令日志需要用户主动展开。
- 安装版 `TASK-004` 日志页显示折叠按钮“技术详情”，原始日志没有默认展开；实时运行和完成结果下均保持折叠。
- 前端新增状态回归测试，防止后续改动再次把原始技术日志默认展开。

### P1-05 已修复：精确提示并安全清理失效 worktree 注册

- Git 兼容性报告解析 `git worktree list --porcelain` 的 `prunable` 块，列出精确失效路径；项目设置和新建任务入口会提示，但不会把仍存在的工作目录误报为失效。
- 清理命令只执行 `git worktree prune`，不运行目录删除；界面明确说明不会删除项目源文件或仍存在的工作目录。
- 安装版先列出 3 个失效注册：`wt/p1-019f7a2c/t6-019f84e2`、`wt/p1-019f7a2c/t7-019f84f8`、`wt/p1/t1`。点击“安全清理失效注册”后提示“项目文件未被删除”，三条失效注册消失。
- Shell 复核显示 `/Users/sapiece/innovationProject/test` 主工作树和 `TASK-001` 至 `TASK-007` 的 7 个现存隔离工作树仍全部注册；没有任何 `prunable` 项。三个旧路径在清理前已经不存在，清理仅移除了 Git 元数据。

### 最终验证与安装物

- `scripts/verify.sh` 全门禁通过：Rust 184 项通过、3 项依赖真实 GitHub/SSH 环境的测试按设计忽略；前端 215 项通过，0 失败；格式检查、workspace Clippy `-D warnings` 和 TypeScript 类型检查均通过。
- Tauri 生产构建、`.app` 和 DMG 打包通过；本机测试副本安装到 `/Applications/AgentFlow.app`，`codesign --verify --deep --strict` 通过。
- 安装版主程序 SHA-256：`1971999404e6b5153ad6ab150562d17b5c8504f7795ab7a58f99fda9fd49431a`；守护进程 SHA-256：`7e887d4d6c31a786fb6cf8559738c1a2cee0220bad4e4bde5f8616db8aa4f1c1`；DMG SHA-256：`2ad2dfe2509396baacf0474e9a3645898dab3e94bf18d13fd4e758b357e383d1`。
- `/Users/sapiece/innovationProject/test` 主工作区 `git status --short` 与 `git diff --check` 均无输出。`TASK-004` 仅在 AgentFlow 数据库中从失败状态恢复为等待计划审批，没有批准计划或执行开发。
- 本轮替换的安装版均移动到废纸篓中的带时间戳备份，可恢复；没有直接删除旧应用。

### P1 最终结论

- P1-01 至 P1-05 均已完成代码修复、自动化门禁和安装版实时操作闭环，可以从本清单的待修复项关闭。
- P0-03 的 Developer ID 签名与 notarization 仍是独立外部发布凭据问题；本轮本机 ad-hoc 重签只用于开发测试，不改变正式分发仍被 Gatekeeper 拒绝的结论。

## P2 全量修复与安装版复测（2026-07-22 18:31）

### P2-01 已修复：非法返工轮数保留草稿并明确校验

- “最大返工轮数”改为字符串草稿状态，不再依赖浏览器数字输入框的隐式正规化；用户可以先输入，再在失焦或提交时看到校验结果。
- 前端只接受完整的 `1–20` 整数；`0`、`-1`、空值和小数都会保留原输入并显示原因，不会被静默改成 `1`、`11` 或其它数字。
- 创建按钮在输入无效时保持禁用；Rust 编排层同时执行 `1–20` 的服务端校验，避免 CLI、旧客户端或绕过前端的调用写入非法值。
- 安装版实测：输入 `0` 后字段仍为 `0`，显示“最大返工轮数必须在 1–20 之间”；输入 `-1` 后字段仍为 `-1`，显示“最大返工轮数必须是 1–20 的整数”。

### P2-02 已修复：结构化验收条件贯穿创建、执行证据与最终批准

- 新建任务支持最多 20 条独立验收条件，可逐条新增、删除和选择“构建 / 测试 / 行为 / 人工验收”，单条正文限制为 1–500 个字符。
- 新增持久化表 `task_acceptance_criteria`；创建任务和验收条件在同一事务内写入，任务详情按固定位置顺序读取。服务端拒绝空条件、超长条件和同类型重复条件。
- 计划、开发、审查提示都注入同一份结构化验收条件，避免只在前端展示、Agent 实际不可见。
- 概览页和审查页逐条展示真实证据状态：构建/测试只依据验证结果，行为条件依据独立审查结果，跳过验证明确显示“未运行验证”，人工条件保持“待人工确认”。
- 最终批准弹窗会再次列出全部验收条件；人工条件必须逐条勾选后才允许批准，不会把自动证据或人工确认伪造成已通过。
- 安装版通过界面执行了新增、类型切换、删除临时条件和创建草稿的完整交互。生成 `TASK-008`，保存四条条件：`build`“生产构建成功”、`test`“自动化测试全部通过”、`behavior`“验收条件可在任务页面逐条回显”、`manual`“人工确认非法轮数输入不会被改写”。
- `TASK-008` 的概览页和审查页均显示 4 条条件及“待验证 / 待人工确认”，任务信息显示“4 条结构化条件”；SQLite 只读复核确认四条记录的类型、位置和正文均正确持久化。
- 当前没有处于 `WAITING_FOR_HUMAN_APPROVAL` 的现有任务，因此没有为了打开最终批准弹窗而启动 Agent 或制造项目改动；最终批准的逐条显示和人工勾选门禁由前端状态测试、TypeScript 检查和生产构建覆盖。这个边界保留在报告中，不把未执行的安装版最终点击描述成已实测。

### 最终验证与安装物

- `scripts/verify.sh` 全门禁通过：Rust 185 项通过、3 项依赖真实 GitHub/SSH 环境的测试按设计忽略；前端 219 项通过、0 失败；格式检查、workspace Clippy `-D warnings` 和 TypeScript 类型检查均通过。
- P2 定向测试通过：编排层覆盖结构化条件持久化和非法返工上限；前端覆盖非法输入草稿、四类证据映射及人工确认语义。
- Tauri 生产构建、`.app` 和 DMG 打包通过；本机测试副本安装到 `/Applications/AgentFlow.app`，`codesign --verify --deep --strict` 通过。
- 安装版主程序 SHA-256：`32f34f8a045fac15c33f82e55d9581c7808e54f7eef87718ee38adf05c9789ce`；DMG SHA-256：`b1afb20355b3ea572c7d405cc20cd4a1f252e7d418430eb0f6cad6b193f8c714`。
- `/Users/sapiece/innovationProject/test` 主工作区 `git status --short` 无输出；`TASK-008` 保持 `DRAFT`，未创建新 worktree、未启动 Provider、未修改项目文件。仓库当前仍是主工作树加 `TASK-001` 至 `TASK-007` 的 7 个既有隔离工作树。
- 修复前安装版保留在 `/Applications/AgentFlow.app.pre-p2-20260722-182604`，可直接恢复。

### P2 最终结论

- P2-01 和 P2-02 均已完成数据模型、后端协议、桌面交互、自动化门禁和安装版实时操作验证，可以从本清单的待修复项关闭。
- 本轮安装仍是本机开发测试用 ad-hoc 签名；P0-03 的 Developer ID 签名和 notarization 外部凭据问题没有被混入 P2 结论。
