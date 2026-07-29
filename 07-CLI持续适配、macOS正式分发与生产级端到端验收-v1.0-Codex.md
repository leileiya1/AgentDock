# AgentDock CLI 持续适配、macOS 正式分发与生产级端到端验收

> 版本：v1.0
> 日期：2026-07-23
> 适用仓库：`/Users/sapiece/innovationProject/AgentDock`
> 测试项目：`/Users/sapiece/innovationProject/test`
> 状态：CLI 扩展与 macOS 分发操作手册已完成；安装版生产级端到端链路、交付终态展示修复与重新安装复测均已通过，详见第 9 节。

## 1. 目标和边界

本文解决三个长期问题：

1. Claude Code、Codex 或后续 Qoder、Trae、Cursor、Grok 等 CLI 升级或新增时，如何以可维护方式接入 AgentDock。
2. AgentFlow 如何从本机 ad-hoc 测试签名升级为可公开分发的 Developer ID 签名和 Apple notarization。
3. 每个准备发布的版本如何真实跑完“创建任务 → 计划审批 → 开发 → 验证 → 独立审查 → 人工批准 → 安全合并”。

本文不把“命令能启动”当作适配完成，也不把单元测试代替真实 CLI 和安装版 UI 验收。正式支持必须同时具备：

- 明确版本和参数契约；
- 无副作用能力检测；
- 隔离目录中的最小真实探针；
- 规划、开发或审查所需的结构化结果；
- 权限、网络、环境变量和数据外发边界；
- 失败分类和有界降级；
- 固定版本 CI、latest 漂移预警和安装版 E2E。

## 2. 当前 Provider 架构基线

### 2.1 已有两种接入方式

| 方式 | 适用情况 | 优点 | 当前限制 |
|---|---|---|---|
| 内置 Rust Adapter | 需要参与规划、开发、审查；CLI 参数和输出需要深度适配 | 能使用完整任务状态机、预算、恢复、实时日志和真实探针 | 每新增一种 CLI 都需要修改并发布 AgentFlow 核心 |
| 外部 Provider sidecar | 独立团队维护、希望热插拔，或只需要开发/审查 | Provider ID 可动态发现；协议、权限和隔离可独立验证 | 当前 Provider Protocol 1.2 只有 Development/Review 结果，尚无 PlanResult；开启计划审批的任务不能用外部 Provider 作为开发首选 |

建议：

- Qoder、Trae、Cursor、Grok 如果有稳定、非交互、可输出结构化结果的官方 CLI，首个受支持版本先走内置 Adapter。
- 如果某 CLI 更新频繁或由第三方单独维护，可以在内置适配稳定后抽成外部 sidecar。
- 在 Provider Protocol 增加 Planning 结果前，不能用 UI 自动化、复制终端文本或伪造 PlanResult 绕过计划门禁。

### 2.2 当前关键文件

| 层级 | 文件 | 职责 |
|---|---|---|
| 稳定 Provider ID | `crates/contracts/src/providers.rs` | `AgentKind`、能力、执行位置、数据外发、权限和信任级别 |
| 项目设置 | `crates/contracts/src/settings.rs` | CLI 路径覆盖、开发/审查降级顺序 |
| 支持矩阵 | `config/provider-compatibility.json` | 固定版本、包名、关键参数和 runtime probe 策略 |
| 矩阵说明 | `config/README.md` | verified / compatible_untested / unsupported 语义 |
| CLI Adapter | `crates/agent-adapters/src/cli_providers.rs` | 参数、权限模式、启动和结果收集 |
| 检测与探针 | `crates/agent-adapters/src/support.rs` | 路径解析、版本、帮助、认证、真实运行探针和错误分类 |
| Adapter 测试 | `crates/agent-adapters/src/tests.rs` | 参数、解析、版本、认证、探针和安全环境回归 |
| Provider 目录 | `crates/orchestrator/src/lifecycle.rs` | 把检测结果变成桌面 Provider 清单 |
| Preflight | `crates/orchestrator/src/preflight.rs` | 并发探针、缓存、角色降级链和阻断结论 |
| 任务调度 | `crates/orchestrator/src/planning.rs`、`development.rs`、`review.rs` | 按角色选择 Provider、记录降级和运行结果 |
| 外部协议 | `crates/provider-protocol/` | NDJSON JSON-RPC、manifest、权限请求、结构化结果和一致性测试 |
| 桌面设置 | `apps/desktop/src/components/ProviderCatalog.tsx`、`routes/settings/ProviderSection.tsx` | 安装、认证、版本、路径、能力和故障展示 |
| 固定版本 CI | `.github/workflows/provider-compatibility.yml` | 安装固定版本并校验参数；每周探测 latest 漂移 |
| 契约脚本 | `scripts/verify-provider-cli-contracts.sh` | `--version`、`--help` 和关键 flag 校验 |

## 3. 新增一种 CLI 的标准流程

### 3.1 第 0 阶段：先确认它真的适合接入

接入前必须记录以下事实：

- 官方 CLI 名称、下载来源、许可证和维护方；
- macOS/Linux/Windows 支持情况与 CPU 架构；
- 是否支持非交互 prompt；
- 是否支持指定工作目录；
- 是否有只读、受限写入或 sandbox 模式；
- 是否能禁用 MCP、插件、用户配置和记忆；
- 是否能以 JSON/NDJSON 输出事件；
- 是否能使用 JSON Schema 或其它稳定结构化输出；
- 是否有无副作用的登录状态命令；
- 是否能区分登录失败、额度、限流、网络、参数错误和模型错误；
- 是否允许审查模式彻底禁用写入；
- 是否能限制最大轮次、时间、token 或费用；
- 是否存在可安全恢复的会话 ID，以及会话 ID 是否会泄露敏感信息。

如果只有 GUI，没有官方非交互 CLI，不应把 GUI 自动点击包装成 Provider。GUI 自动化无法稳定提供权限、结构化输出、超时、取消和工作目录隔离。

### 3.2 第 1 阶段：定义稳定身份和能力

内置 Provider：

1. 在 `AgentKind` 增加稳定枚举，例如 `CursorCli`。
2. `as_str()` 使用长期稳定且不会随产品改名变化的 ID，例如 `cursor_cli`。
3. 更新 `FromStr` 和生成的 TypeScript bindings。
4. 在 `ProjectSettings` 增加可选绝对路径覆盖，例如 `cursor_path`。
5. 明确 `AgentCapabilities`：
   - `supports_development`
   - `supports_review`
   - `read_only_mode`
   - `streams_events`
   - `native_output_schema`
   - `supports_resume`

外部 Provider：

1. 使用 2–64 字符的小写稳定 ID。
2. manifest 必须声明 execution location、data egress、最小文件/网络/命令权限。
3. Provider Protocol 版本必须与核心协商一致。
4. 先通过 `crates/provider-protocol/tests/conformance.rs` 同等级测试，再允许加载。

### 3.3 第 2 阶段：加入唯一支持矩阵

在 `config/provider-compatibility.json` 增加：

```json
{
  "cursor": {
    "displayName": "Cursor CLI",
    "package": "官方包名或 null",
    "verifiedVersions": [],
    "requiredFlags": ["实际依赖的参数"],
    "runtimeProbe": null
  }
}
```

规则：

- `verifiedVersions` 初始必须为空。
- 只有固定版本通过真实运行和全量回归后，才能加入 verified。
- `requiredFlags` 只放 AgentFlow 实际安全依赖的参数，不能把所有帮助参数复制进去。
- runtime probe 未实现时保持 `null`；此 Provider 可以显示检测信息，但不能进入真实自动降级链。
- 包名必须来自官方注册源；没有官方包时保持 `null`，不允许安装同名未知包。

### 3.4 第 3 阶段：实现路径解析和静态能力检测

检测必须满足：

- 用户配置的绝对路径优先；
- 其次检查官方应用内置 CLI 路径；
- 最后使用 AgentFlow 的显式 PATH 候选；
- 预检、认证和真实启动必须复用同一个解析函数；
- 不允许出现“预检命中 A，真实运行命中 B”；
- 解析结果记录真实路径和版本，供 UI 与审计显示。

静态检测只允许执行：

- `--version`；
- `--help` 或目标子命令 `--help`；
- 官方无副作用认证状态命令。

每条命令必须有独立超时，stdout/stderr 必须限长，不能继承整个桌面进程环境。

### 3.5 第 4 阶段：实现认证检测

认证状态必须是三态：

- `Some(true)`：官方命令明确证明已登录；
- `Some(false)`：官方命令明确证明未登录或认证过期；
- `None`：上游没有稳定无副作用认证命令，不能猜测。

注意：

- `None` 不等于未登录；应由真实探针进一步判断。
- 最小环境至少要保留 CLI 读取系统账号所需的 `HOME`、`USER`、`LOGNAME` 和受控 PATH。
- 不继承未知 API key、代理变量或 shell 初始化脚本。
- 认证详情只显示账号类型或非敏感标签，不能记录 token、cookie、session 和密钥值。

### 3.6 第 5 阶段：实现隔离的最小真实探针

探针目标不是测试模型能力，而是证明“这个版本、账号、参数和结构化输出今天能跑”。

强制要求：

1. 使用临时目录并初始化最小 Git 仓库。
2. 工作目录与真实项目完全隔离。
3. 禁止工具调用、网络工具、插件、MCP、记忆和用户配置（若 CLI 提供对应参数）。
4. 使用只读/sandbox 模式。
5. prompt 只要求返回固定 marker，例如 `AGENTFLOW_PROBE_OK`。
6. 优先使用 JSON Schema 约束固定对象。
7. stdin 关闭，不能进入交互登录。
8. 45 秒绝对超时；进程退出或超时必须回收进程树。
9. 探针结果按 Provider、绝对路径、版本、认证方式缓存 10 分钟。
10. 原始输出只用于本地分类，不进入数据库或普通 UI。

每个 CLI 都必须有自己的探针策略。不能把 Claude 或 Codex 的参数模板复制给其它产品。

### 3.7 第 6 阶段：实现 Adapter

每个 Adapter 至少实现：

- `kind()`：稳定 Provider ID；
- `detect()`：解析路径、版本、关键参数和能力；
- `capabilities()`：角色、流式、schema、只读和 resume；
- `start()`：构造 argv 和最小环境，通过 process supervisor 启动；
- `collect_result()`：只接受稳定结构化结果。

角色隔离：

- Planner：只读仓库，只输出计划；
- Developer：只允许任务工作树内受控写入；
- Reviewer：必须只读，绑定当前 revision 的 commit SHA；
- Validator：由 AgentFlow 运行项目命令，不由模型声称“测试通过”。

结果解析规则：

- 优先读取专用结果文件，不从最后一行自然语言猜结果；
- schema 版本、task ID、revision 和 commit SHA 必须校验；
- 允许修复 Provider 包装层，但不能把无效自然语言补成虚构结果；
- Reviewer 的 pass/request_changes/block 必须绑定真实 issue 和证据；
- 解析失败最多进行一次只读修复，然后进入后备 Provider。

### 3.8 第 7 阶段：权限与安全边界

新 CLI 必须经过与 Claude/Codex 相同的权限代理：

- 工作树内读写；
- 工作树外路径；
- 命令执行和参数；
- 依赖安装；
- 网络域名、端口和协议；
- 环境变量名称；
- Git mutation；
- 进程控制和系统变更。

禁止事项：

- 永久 `fullAccess=true`；
- Reviewer 写入；
- shell 字符串拼接代替 argv；
- 允许 `sudo`、系统目录写入或任意外部路径；
- 把密钥写入 prompt、argv、日志或数据库；
- 把模型的授权文本当成人类批准；
- 把 `sandbox-exec` 异常退出当作已实现强隔离。

高风险 Provider 在没有可验证 sandbox 时，应明确显示保证级别并限制为只读审查，不能用“受限沙箱”文案掩盖实际能力。

### 3.9 第 8 阶段：接入 Provider 目录和降级链

需要更新：

- `env_check()` 的并发静态检测；
- `provider_list()` 的桌面目录项；
- `cli_probe_target()`；
- `ProjectSettings` 默认和可编辑降级顺序；
- 新建任务开发/审查下拉框；
- Provider 图标、安装入口和认证说明；
- 设置页的实际路径、版本、支持等级和问题详情。

降级规则：

- 开发和审查按角色维护独立有序链；
- 同一 Provider 家族不能既开发又独立审查；
- 未安装、未认证、缺少 runtime probe 或探针失败的 Provider 不进入 ready 链；
- 额度、限流、认证、参数漂移、schema、超时分别记录；
- Provider 故障不等于审查反对票；
- 所有跳过和尝试原因进入时间线，最终错误汇总完整链路。

### 3.10 第 9 阶段：测试矩阵

最低自动测试：

1. 版本解析：稳定版、预发布版、异常输出。
2. required flags：齐全和缺失。
3. 路径优先级：显式路径、官方内置路径、PATH、多版本冲突。
4. 最小环境：保留账号上下文但不继承密钥。
5. 认证：已登录、未登录、未知、命令超时。
6. runtime probe：成功、额度、限流、认证、参数漂移、schema、45 秒超时。
7. Planner/Developer/Reviewer argv 快照。
8. Reviewer 只读和 Developer 工作树边界。
9. 结构化结果：正确、包装、截断、错误 task/revision/commit、恶意文本。
10. 取消、空闲超时、绝对超时和进程树回收。
11. 降级链顺序、同源审查排除、并发 slot 和 RPM。
12. 权限请求、拒绝、过期、恢复和 daemon 重启。
13. 桌面状态文案、路径/版本详情、失败恢复按钮和无障碍。
14. 安装版真实 CLI E2E。

固定版本进入 stable 前必须运行：

```bash
scripts/verify-provider-cli-contracts.sh
cargo test -p agentflow-agent-adapters
cargo test -p agentflow-provider-protocol
cargo test -p agentflow-orchestrator preflight --lib
cd apps/desktop && bun test
cd ../.. && scripts/verify.sh
```

### 3.11 第 10 阶段：CI 和发布节奏

固定版本 CI：

- 安装精确版本；
- 校验 `--version` 和 required flags；
- 运行 Adapter/协议测试；
- 失败时禁止合并支持矩阵变更。

每周 latest advisory：

- 只检测上游漂移并报警；
- 不自动改 `verifiedVersions`；
- 不自动更新用户电脑；
- 不因 latest 通过就删除旧 stable；
- 需要人工完成真实探针和 E2E 后再提升版本。

建议保留：

- 当前 stable；
- 上一个 stable，作为紧急回退；
- latest，只用于预警，不进入生产调度。

## 4. Qoder、Trae、Cursor、Grok 的接入顺序

### 4.1 第一批：有正式非交互 CLI 的产品

只有在官方文档证明支持非交互、工作目录、只读/写入控制和可解析输出后，才进入 Adapter 开发。

优先级建议：

1. Cursor CLI：先确认官方 CLI 与编辑器 Agent 模式的边界，禁止依赖 GUI 会话。
2. Qoder CLI：确认包名、许可证、登录状态和 headless 输出；不要安装拼写相近的第三方 npm 包。
3. Trae CLI：确认是否存在公开稳定的非交互协议；若只有 IDE，不接入自动执行链。
4. Grok CLI：当前矩阵已有占位，但包名、固定版本和 runtime probe 都为空，保持不可运行，直到官方契约齐全。

产品顺序不能代替安全门槛。任一产品缺少只读模式或稳定结果契约时，只能保留“发现/未支持”状态。

### 4.2 每个产品必须填写的适配卡

```text
Provider ID:
官方产品名:
官方仓库/安装源:
许可证:
固定版本:
支持系统/架构:
版本命令:
帮助命令:
认证状态命令:
非交互 prompt 参数:
工作目录参数:
只读/沙箱参数:
禁用插件/MCP/用户配置参数:
JSON/NDJSON 参数:
结构化 schema 支持:
开发能力:
审查能力:
恢复能力:
预算能力:
网络需求:
最小环境:
成功 marker:
超时:
已知错误分类:
固定版本 CI:
真实 E2E 证据:
回退版本:
维护人:
```

## 5. CLI 升级和漂移处理 SOP

### 5.1 收到 latest 预警后

1. 保存上游版本、帮助输出和失败日志摘要。
2. 对比 required flags、默认权限、输出 schema 和认证命令。
3. 在隔离机器或临时用户环境安装候选版本。
4. 运行最小真实探针，不操作真实项目。
5. 运行 Planner、Developer、Reviewer 三角色契约测试。
6. 跑一个仅修改测试仓库的完整 E2E。
7. 如果全部通过，把版本加入 `verifiedVersions` 并更新固定版本 CI。
8. 如果不通过，保留当前 stable，在 UI 标记候选版本“参数兼容但未经回归”或“不兼容”。

### 5.2 必须立即阻断的漂移

- 删除只读/sandbox 参数；
- 默认启用插件、MCP、记忆或外部工具且无法关闭；
- 结构化输出不再稳定；
- 认证命令开始进入交互；
- 工作目录参数失效；
- Reviewer 可以写文件；
- 退出码不再区分成功与失败；
- 会话恢复 token 不再是不可见、不透明值；
- CLI 未经允许向额外域名发送项目数据。

### 5.3 回退

- 支持矩阵继续保留上一个 verified 版本。
- 安装入口提供固定版本，而不是始终安装 latest。
- 真实运行前发现版本漂移时从本次链移除，不卸载用户软件。
- 已开始任务只能从持久化检查点安全恢复到可用 Provider，不能跨过计划或人工批准。

## 6. CLI 适配完成定义（Definition of Done）

某 CLI 只有同时满足以下条件才可标记“已连接 · 已验证”：

- [ ] 官方来源和许可证明确。
- [ ] 固定版本和回退版本明确。
- [ ] required flags 自动验证。
- [ ] 预检与运行解析同一绝对路径。
- [ ] 无副作用认证检测或明确的 unknown 语义。
- [ ] 独立的真实 runtime probe。
- [ ] Planner/Developer/Reviewer 所需角色经过验证。
- [ ] Reviewer 强制只读。
- [ ] 结构化输出严格绑定 task/revision/commit。
- [ ] 取消、超时和进程树回收通过。
- [ ] 权限代理和数据外发审批通过。
- [ ] 降级链及错误汇总通过。
- [ ] 固定版本 CI 与 latest advisory 已配置。
- [ ] Rust、前端、协议和安装版 E2E 通过。
- [ ] 文档包含已知限制、升级和回退步骤。

## 7. 获得 Developer ID 签名和 Apple notarization

### 7.1 先选择账号类型

个人/个体经营：

- Apple Account 开启双重认证；
- 使用真实法定姓名；
- 达到所在地区法定年龄；
- 不需要 D‑U‑N‑S；
- 软件签名会显示个人身份。

组织：

- Apple Account 开启双重认证，建议使用组织域名邮箱；
- 申请人必须有权代表组织签署法律协议；
- 组织必须是可签约法律实体；
- 需要免费的 D‑U‑N‑S Number；
- 需要公开可用、与组织关联的官网和工作邮箱；
- Apple 可能要求额外商业文件或电话核验。

Apple Developer Program 当前官方价格为每个会员年度 99 美元，地区可能以当地货币显示。加入时需要由你本人/组织授权人确认法律协议并支付；这一步不能由代码或 CI 代替。

办理入口：<https://developer.apple.com/programs/enroll/>

### 7.2 创建 Developer ID Application 证书

会员审核通过后：

1. 登录 Apple Developer 的 Certificates, Identifiers & Profiles。
2. 打开 Certificates，点击 `+`。
3. 选择 `Developer ID`。
4. 选择 `Developer ID Application`，用于签名 `.app`。
5. 通过“钥匙串访问 → 证书助理 → 从证书颁发机构请求证书”生成 CSR。
6. 上传 `.certSigningRequest`。
7. 下载 `.cer` 并双击安装到 login keychain。
8. 在“钥匙串访问 → 我的证书”中确认该证书下面存在私钥。
9. 运行：

```bash
security find-identity -v -p codesigning
```

必须看到类似：

```text
Developer ID Application: 法定姓名或组织名 (TEAMID)
```

只有 Account Holder 能创建普通 Developer ID 证书；Apple 官方目前允许最多五个 Developer ID Application 和五个 Developer ID Installer 证书。AgentFlow 直接分发 DMG 时主要需要 Developer ID Application；只有发布 `.pkg` 安装器时才需要 Developer ID Installer。

官方步骤：<https://developer.apple.com/help/account/certificates/create-developer-id-certificates/>

### 7.3 导出供 GitHub Actions 使用的证书

1. 在“钥匙串访问 → 我的证书”展开 Developer ID Application。
2. 选中证书及其私钥，导出 `.p12`。
3. 为 `.p12` 设置一个独立强密码。
4. 转成单行 base64：

```bash
openssl base64 -A -in /安全位置/DeveloperID.p12 -out /安全位置/DeveloperID.p12.base64
```

5. 不要把 `.p12`、base64 文件或密码提交到仓库。

### 7.4 获取 notarization 凭据

当前仓库 `.github/workflows/release-macos.yml` 已使用 Apple ID 方式，需要六个 GitHub Secrets：

| Secret | 内容 |
|---|---|
| `APPLE_CERTIFICATE` | `.p12` 的单行 base64 |
| `APPLE_CERTIFICATE_PASSWORD` | 导出 `.p12` 时设置的密码 |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: ... (TEAMID)` 完整名称 |
| `APPLE_ID` | Apple Account 邮箱 |
| `APPLE_PASSWORD` | 不是主密码；必须是 app-specific password |
| `APPLE_TEAM_ID` | Apple Developer Membership 页面中的 Team ID |

生成 app-specific password：

1. 登录 <https://account.apple.com/>。
2. 打开“登录与安全 / Sign-In and Security”。
3. 选择“App-Specific Passwords”。
4. 生成一个只用于 AgentFlow notarization 的密码。

Apple Account 必须启用双重认证。修改或重置 Apple Account 主密码会撤销所有 app-specific passwords，需要同步更新 GitHub Secret。

Apple 官方说明：<https://support.apple.com/102654>

更适合长期 CI 的替代方案是 App Store Connect API key：

- `APPLE_API_ISSUER`
- `APPLE_API_KEY`
- `APPLE_API_KEY_PATH`

API 私钥 `.p8` 只能下载一次。若采用此方案，应先修改 workflow，在临时目录从 GitHub Secret 恢复 `.p8`，构建结束立即清理；不要同时混用 Apple ID 和 API key 两套认证。

### 7.5 配置 GitHub Secrets

在 GitHub 仓库：

`Settings → Secrets and variables → Actions → New repository secret`

逐项加入以上六个 Secret。注意：

- 不要在 PR 日志中打印 Secret。
- fork PR 不应获得发布 Secret。
- release job 只在受保护 tag 或人工批准环境运行。
- 最好把 Secrets 放进受保护的 `release` Environment，并要求维护者批准。
- 证书泄露时立即在 Apple Developer 后台撤销，并轮换 `.p12`、密码和 CI Secret。

### 7.6 本地签名和 notarization 验证

Developer ID 构建必须满足：

- 所有可执行文件均签名；
- Hardened Runtime；
- secure timestamp；
- 不含 `com.apple.security.get-task-allow=true`；
- 使用 Developer ID Application，而不是 ad-hoc、Apple Development 或 Mac Distribution。

构建后执行：

```bash
codesign --verify --deep --strict --verbose=2 \
  apps/desktop/src-tauri/target/release/bundle/macos/AgentFlow.app

codesign -dv --verbose=4 \
  apps/desktop/src-tauri/target/release/bundle/macos/AgentFlow.app

spctl --assess --type execute --verbose=4 \
  apps/desktop/src-tauri/target/release/bundle/macos/AgentFlow.app

hdiutil verify \
  apps/desktop/src-tauri/target/release/bundle/dmg/AgentFlow_0.1.0_aarch64.dmg

xcrun stapler validate \
  apps/desktop/src-tauri/target/release/bundle/macos/AgentFlow.app

xcrun stapler validate \
  apps/desktop/src-tauri/target/release/bundle/dmg/AgentFlow_0.1.0_aarch64.dmg

scripts/verify-macos-bundle.sh
```

预期：

- Authority 为 Developer ID Application；
- `spctl` 为 accepted；
- notarization ticket 存在且 stapler validate 通过；
- DMG 完整；
- SHA-256 与发布记录一致。

Apple 已停止接受 `altool` 上传；必须使用 Xcode 14+ 的 `notarytool` 或 Notary API。官方说明：<https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>

### 7.7 仓库发布流程

当前 release workflow 在 tag `v*` 或手动运行时：

1. 安装 Rust/Bun 依赖；
2. 构建 sidecar 和前端；
3. Tauri 使用 Developer ID 签名；
4. 使用 Apple 凭据提交 notarization；
5. stapling；
6. `verify-macos-bundle.sh` 执行 codesign、spctl、hdiutil 和 SHA-256 门禁；
7. 任一步失败都拒绝发布。

首次正式发布建议先使用 `workflow_dispatch`，确认完整门禁和产物，再创建正式 tag。

## 8. 生产级端到端验收 SOP

### 8.1 测试任务要求

- 只使用专门测试仓库；
- 主分支开始前干净；
- 改动规模小、可人工检查、可安全合并；
- 不安装依赖、不访问外部服务、不涉及密钥；
- 至少包含行为、自动测试和人工三类验收条件；
- 开发和审查使用不同 Provider 家族；
- 开启计划审批；
- 使用本地安全合并；
- 最终必须验证 main、worktree、任务状态和审计事件。

### 8.2 完整步骤和证据

| 阶段 | 操作 | 必须保存的证据 |
|---|---|---|
| 基线 | 检查主分支和工作区 | branch、HEAD、`git status --short` |
| 环境 | 安装版重新检测 | 系统、CLI 路径、版本、认证和真实探针 |
| 创建 | 填写范围、预算、三类验收条件 | TASK 编号、Provider 组合、策略快照 |
| Preflight | 并发探测开发/审查链 | 每个 Provider 可用/跳过原因 |
| 规划 | Planner 只读生成计划 | run、Provider、退出码、结构化计划 |
| 计划审批 | 人工核对文件范围和验证命令 | 批准事件和计划版本 |
| 开发 | Developer 只改隔离工作树 | worktree、branch、commit、diff |
| 验证 | AgentFlow 运行真实命令 | 命令、退出码、通过/失败计数 |
| 审查 | 独立 Reviewer 绑定 commit | decision、issues、证据和 Provider |
| 返工 | 若 request_changes，进入下一 revision | issue 关闭/复现和新 commit |
| 最终批准 | 人工检查 diff、风险和验收条件 | 自动证据状态、人工勾选、批准事件 |
| 合并 | 本地安全合并 | main 新 HEAD、祖先关系、任务 `MERGED` |
| 收尾 | 检查工作区和 worktree | main 干净、无 prunable、审计可导出 |

### 8.3 失败判定

以下任一情况使 E2E 不通过：

- 预检和真实运行使用不同 CLI 路径或版本；
- 计划批准前写入项目；
- Developer 写出任务范围外文件；
- Reviewer 修改文件；
- 自动验证未运行却显示通过；
- 审查未绑定当前 commit；
- 人工验收未勾选仍可批准；
- merge 前后 SHA/branch 不一致；
- 主工作区遗留未提交修改；
- Provider 失败没有进入降级链或没有清楚原因；
- 安装版 UI 状态与数据库状态不一致。

## 9. 2026-07-23 安装版生产级 E2E 实测记录

### 9.1 基线和任务设计

- 安装版：`/Applications/AgentFlow.app`
- 测试项目：`/Users/sapiece/innovationProject/test`
- 基线分支：`main`，最终有效任务的基线为 `5181cc3a828e043f4e7cc827329ff901d8ce15df`
- 基线工作区：干净
- 最终开发 Agent：Claude Code
- 最终审查委员会：Codex + DeepSeek API，最小成功数 2
- 计划审批：开启
- 交付：本地安全合并
- 改动范围：仅新增 `package.json`、`src/providerResult.ts`、`src/providerResult.test.ts`
- 网络和依赖安装：禁止
- 验收条件：功能行为、`bun test`、人工文件范围核对
- 项目验证配置：`bun test`，120 秒超时
- 项目只读命令补充：`bun test`、`bun --version`、`git ls-tree`
- 当前项目配置提交：`5181cc3 test: allow read-only CLI probes`
- 当前项目配置 SHA-256 信任值：`6c7d3d0de3faacacae6bed0f3442da5c12f039ce02dbea83275caa16303f4022`

### 9.2 实测结果

结论：**代码交付核心链路通过；发现的交付终态展示问题已修复，并在重新打包安装的 `/Applications/AgentFlow.app` 上复测通过。按第 8.3 节的 UI/数据库一致性标准，本轮生产级端到端验收通过。**

最终成功任务：

| 证据 | 实测值 |
|---|---|
| 任务 | `TASK-013`，生产级端到端验收：受限 Claude 命令闭环 |
| Task ID | `019f8c94-dccf-7113-84a9-9cb9c0664478` |
| 计划 | 只读生成并经人工批准 |
| Developer | Claude Code，成功，未进入后备 Provider |
| 开发 commit | `00710de09ea88dd5678330f73d33670203ee6067` |
| AgentFlow 验证 | `bun test`，6 pass、0 fail、6 expects |
| Reviewer 1 | Codex，pass |
| Reviewer 2 | DeepSeek API，pass |
| 审查委员会 | 2/2 返回，质量分 100，0 个独立问题 |
| 人工批准 | 已逐条核对验收条件并批准 commit `00710de…` |
| 交付 | 本地安全合并成功 |
| main 合并 commit | `e5adee77cba3ed3cc475d10ec34dafc66464e0d8` |
| 数据库任务状态 | `MERGED` |
| delivery record | `state=merged`、`ci_status=passed` |
| merge 审计事件 | `merge:succeeded`，记录 pre-merge 与 merge commit |
| worktree | TASK-013 工作树已清理，不再出现在 `git worktree list` |
| main 工作区 | 干净 |
| 合并后独立复测 | `bun test`：6 pass、0 fail |

最终 main 只新增 3 个计划内文件，共 `+79/-0`：

```text
package.json
src/providerResult.ts
src/providerResult.test.ts
```

### 9.3 失败样本和根因定位

本轮没有只保留成功截图，还真实验证了安全阻断与降级：

| 任务 | 结果 | 证明了什么 |
|---|---|---|
| `TASK-009` | 未运行任何项目验证；测试文件触发至少 2 个独立 Provider 的审查门禁，未强制批准 | 缺少 `.agentflow/project.toml` 时不会把模型自述当成验证通过；委员会不足会安全阻断 |
| `TASK-010` | Codex 开发 commit `f2082619…`；`bun test` 通过；DeepSeek 审查成功；Claude 审查在动态 `git ls-tree` 命令上报 `PERMISSION_UNSTRUCTURED_PROMPT`，委员会 1/2，任务未获准合并 | Claude 已登录，失败不是账号登录，而是 Adapter 无法把动态 Bash 请求转成精确、结构化、可批准的操作 |
| `TASK-011` / `TASK-012` | Claude 开发先后在 `bun --version`、`git status \| grep ...` 等探测/审计命令触发权限失败，系统降级到 Codex；Codex 成为实际开发者后又被同源独立审查规则排除，委员会无法凑齐 | 降级和同源审查排除有效；仅扩大命令白名单无法覆盖模型临时拼接的 shell 命令 |
| `TASK-013` | 给 Claude 注入明确的角色命令配置：文件检查只用 Read/Glob；禁止管道、重定向及临时 shell 组合；Bash 仅允许独立的 `bun --version`、`bun test`、`git status`、`git diff`、`git log`、`git ls-tree`；通过后立即输出结构化结果 | 受限、确定性的命令契约可让 Claude 开发、项目验证、双独立审查、人工批准和本地合并完整闭环 |

根因判断：Claude Code 的“未登录”提示曾是能力探测环境/错误分类问题；本轮真实执行已经证明终端登录本身有效。当前剩余兼容问题是 Claude 会按上下文临时生成 Bash 组合命令，而 AgentFlow 的权限回调在拿不到精确 operation 时会按设计安全拒绝。这个行为与模型质量无直接关系，属于 CLI 版本行为、Adapter prompt 和权限协议之间的契约漂移。

长期修复不应继续无限增加命令白名单，而应：

1. Adapter 自动为 Planner、Developer、Reviewer 注入不同的命令策略模板。
2. 能用文件 Read/Glob 或原生 Git argv 完成的检查，不允许生成 shell 管道。
3. Provider 请求额外权限时必须返回精确 argv、cwd、作用域和理由；无法结构化时维持拒绝。
4. 权限 UI 只允许批准单一精确操作或受约束的项目规则，不提供任意 shell 放行。
5. 为 Claude 固定版本增加真实 Developer/Reviewer E2E，覆盖 `git status`、`git diff`、`git log`、`git ls-tree` 以及动态 Bash 被拒绝后的可恢复路径。
6. Planner 在只读角色下不应先尝试 Write 再退回自然语言结果；应直接走结构化 PlanResult 通道，减少噪音和误报。

### 9.4 本轮发现并已修复的前端问题

#### P1（已修复）：任务已合并，但详情页交付节点仍显示“进行中”

复现证据：

- 顶部任务状态显示“已合并”；
- 返回任务列表后，`TASK-013` 位于“已完结”，卡片状态为“已合并”；
- SQLite `tasks.status=MERGED`；
- `delivery_records.state=merged`、`ci_status=passed`；
- 已产生 `merge:succeeded` 事件，main 也确实位于 `e5adee77…`；
- 但重新进入详情页后，r1 仍显示“当前轮 进行中”，交付节点显示“已合并到目标分支 进行中”，底部持续显示“正在交付”。

影响：不会破坏代码或重复合并，但会让用户误以为后台仍在执行，违反 UI 与持久化状态一致性门禁。

已定位的直接根因：`apps/desktop/src/copy/events.ts` 把 `human:merge` 映射为 delivery 阶段的 `running`，把后续 `merge:succeeded` 映射为 `ok`；`apps/desktop/src/lib/execution/tree.ts` 的 `worst()` 又按 `running > ok` 聚合同一阶段的全部事件。因此成功事件到达后，较早的 `human:merge=running` 永远压过 `merge:succeeded=ok`。现有 `apps/desktop/src/lib/execution/tree.test.ts` 只验证终态不会补挂 pending 阶段，没有断言 delivery phase、revision 和 live status 都必须结束，所以该组合回归漏测。

已完成的修复：

1. `apps/desktop/src/lib/execution/tree.ts` 改用按时间和事件 ID 排序的阶段状态转换；后到的 `merge:succeeded` 会关闭较早的 `human:merge`。
2. 被后续终态关闭的历史 running 事件转为 `info/已记录`，避免展开交付节点后“开始合并”图标仍然旋转。
3. 成功、失败或人工处理等终态事件会写入真实 `endedAt`，当前终态 revision 的结论改为“已交付”。
4. `apps/desktop/src/lib/execution/liveStatus.ts` 增加终态硬兜底：`MERGED`、`ROLLED_BACK`、`CANCELLED` 永远不返回 running live status。
5. `apps/desktop/src/lib/execution/tree.test.ts` 增加 `human:merge → merge:succeeded`、计划批准关闭等待态，以及陈旧 running 快照遇到 `MERGED` 的回归测试。

验证结果：

- 执行树定向测试：17 pass、0 fail、45 expects；
- 桌面完整测试：221 pass、0 fail、510 expects；
- `tsc --noEmit && vite build`：通过；
- `tauri build --bundles app`：通过；
- 已重新安装 `/Applications/AgentFlow.app`，保留原数据库和项目数据；
- 安装版重新进入 `TASK-013` 后，r1 显示“已交付 / 通过”；
- delivery 显示“已合并到目标分支 / 通过”；
- 历史 `human:merge` 显示“已记录 开始合并”，不再旋转；
- 页面不再显示“正在交付”及持续 elapsed timer。

## 10. 官方参考资料

- Apple Developer Program 加入要求：<https://developer.apple.com/programs/enroll/>
- D‑U‑N‑S：<https://developer.apple.com/help/account/membership/D-U-N-S/>
- Developer ID 证书：<https://developer.apple.com/help/account/certificates/create-developer-id-certificates/>
- macOS notarization：<https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>
- 自定义 notarytool 流程：<https://developer.apple.com/documentation/security/customizing-the-notarization-workflow>
- app-specific password：<https://support.apple.com/102654>
- Tauri macOS 签名与 notarization：<https://v2.tauri.app/distribute/sign/macos/>
