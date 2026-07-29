# Provider 兼容策略

`provider-compatibility.json` 是 AgentFlow 内置 CLI 的唯一支持矩阵。它区分三种状态：

- `verified`：版本号与矩阵中的固定版本完全一致，并且关键参数仍存在。
- `compatible_untested`：关键参数仍存在，但该版本尚未进入固定版本回归；创建任务前必须通过真实探针。
- `unsupported`：缺少 AgentFlow 依赖的参数，不能进入运行链。

环境检查只执行 `--version`、`--help` 和无副作用的登录状态命令。任务预检从实际开发/审查降级链中筛出“已安装、参数兼容、已认证、声明了 `runtimeProbe`”的本地 CLI，并将任意数量的最小真实请求并发执行；未安装、未登录、任务未使用或尚无安全探针协议的 CLI 不会发出真实请求。结果按可执行路径、版本和认证方式缓存 10 分钟。认证、额度、超时或结构化输出失败都会把该 Provider 从本次可运行链移除，让调度器选择后备 Provider。

新增 Qoder、Trae、Cursor、Grok 等 CLI 时，依次补齐：Provider 标识和目录描述、矩阵中的关键参数、非交互只读探针策略及其测试、对应 Adapter。并发调度器不需要随 CLI 数量修改；在安全探针尚未实现前将 `runtimeProbe` 保持为 `null`，不能用一个未经验证的通用命令猜测认证或额度状态。

`requestPolicy` 声明 CLI 内部不可见请求所需的协议兼容策略。Grok 使用 DeepSeek V4 时采用 `deepseek_forced_tool_choice_non_thinking`：AgentFlow 为每次运行创建仅监听回环地址的短生命周期网关，用随机令牌替代真实凭据，并只在请求使用强制 `tool_choice` 时注入 `thinking: {"type":"disabled"}`。主 Agent 的普通推理请求保持 thinking；未知策略必须 fail closed，不能静默套用其他 Provider 的改写规则。

升级固定版本时：先修改矩阵与 `provider-compatibility.yml` 中的精确 npm 版本，运行 `scripts/verify-provider-cli-contracts.sh` 和 Rust/桌面测试，再提交。每周的 latest advisory 只侦测上游参数漂移，不会自动把未经验证的新版本加入稳定矩阵，也不会改写用户电脑上的 CLI。

## Lima OS 隔离

`config/lima/agentdock-isolated.yaml` 是高风险验证使用的固定 Lima 2.2.0 配置。它不挂载任何宿主机目录，不转发 SSH agent 或代理环境。启用节点的“断网验证”后，每条验证命令都会经 root 所有的 `agentflow-offline` 进入独立 Linux network namespace，再以原 SSH 用户和 `no_new_privs` 执行；其中没有 IPv4/IPv6 路由，也不能通过 `sudo` 或 setuid 程序恢复权限。执行 `scripts/setup-lima-isolation.sh` 创建并进行 fail-closed 验证。

该虚拟机是额外执行边界，不表示宿主机上的 CLI Provider 已自动进入隔离环境。只有在虚拟机内独立安装并认证 Provider 后，才能把 Provider 执行迁进去；AgentDock 不会通过挂载宿主机 home 目录偷取凭据。

VM 启动后可在“设置 → 远程执行节点”登记 `127.0.0.1:60022`，用户名使用 Lima 创建的来宾用户名，工作根目录使用 `/tmp/agentflow`，私钥路径使用 `~/.lima/_config/user` 的绝对路径，并开启“断网验证”。数据库只保存路径和策略开关；私钥必须存在且权限不得开放给 group/other。首次登记前应使用 OpenSSH 核对并接受该本机 VM 的 host key。
