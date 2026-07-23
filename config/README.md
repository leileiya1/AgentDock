# Provider 兼容策略

`provider-compatibility.json` 是 AgentFlow 内置 CLI 的唯一支持矩阵。它区分三种状态：

- `verified`：版本号与矩阵中的固定版本完全一致，并且关键参数仍存在。
- `compatible_untested`：关键参数仍存在，但该版本尚未进入固定版本回归；创建任务前必须通过真实探针。
- `unsupported`：缺少 AgentFlow 依赖的参数，不能进入运行链。

环境检查只执行 `--version`、`--help` 和无副作用的登录状态命令。任务预检从实际开发/审查降级链中筛出“已安装、参数兼容、已认证、声明了 `runtimeProbe`”的本地 CLI，并将任意数量的最小真实请求并发执行；未安装、未登录、任务未使用或尚无安全探针协议的 CLI 不会发出真实请求。结果按可执行路径、版本和认证方式缓存 10 分钟。认证、额度、超时或结构化输出失败都会把该 Provider 从本次可运行链移除，让调度器选择后备 Provider。

新增 Qoder、Trae、Cursor、Grok 等 CLI 时，依次补齐：Provider 标识和目录描述、矩阵中的关键参数、非交互只读探针策略及其测试、对应 Adapter。并发调度器不需要随 CLI 数量修改；在安全探针尚未实现前将 `runtimeProbe` 保持为 `null`，不能用一个未经验证的通用命令猜测认证或额度状态。

升级固定版本时：先修改矩阵与 `provider-compatibility.yml` 中的精确 npm 版本，运行 `scripts/verify-provider-cli-contracts.sh` 和 Rust/桌面测试，再提交。每周的 latest advisory 只侦测上游参数漂移，不会自动把未经验证的新版本加入稳定矩阵，也不会改写用户电脑上的 CLI。
