# 任务：实现「项目外观、项目成员、Agent 档案」三组后端接口

你在 AgentDock 仓库（`/Users/sapiece/innovationProject/AgentDock-main`）里工作。这是一个 Tauri 2 桌面应用：Rust 后端加 React 18 + TypeScript 前端。仓库**不是 git 仓库**，不要提交，也不要 init。

先读这些文件，了解背景和现有写法：

- `docs/像素小屋_落地方案_2026-11-06.md`：整体方案。重点读「项目小镇与建筑类型」「后端项目汇总」「尚未决定」三节。
- `crates/orchestrator/src/project_summary.rs` 和 `crates/orchestrator/src/task_queries.rs` 里的 `project_summaries`：最近一个同类接口，照它的风格写。
- `crates/contracts/src/settings.rs` 里的 `ProjectSettings`，以及它的整条写入路径：
  - `apps/desktop/src-tauri/src/main.rs` 的 `project_settings_update`；
  - `DaemonRequest::ProjectSettingsUpdate`；
  - `crates/daemon/src/lib.rs` 的处理逻辑。
  - **所有写操作都按这条路径走 daemon**，读操作可以直接查。
- `crates/contracts/src/providers.rs` 的 `AgentKind`：它的序列化方式是原始字符串，`External(String)` 也要能存取。
- `crates/persistence/migrations/`：现有迁移到 `0026`，新迁移从 `0027` 开始编号。
- 前端已经预留好的类型：
  - `apps/desktop/src/lib/studio/buildings.ts`：`ProjectLook`、`YardItem`、`PROJECT_KINDS`、`buildingFor`；
  - `apps/desktop/src/lib/studio/townResidents.ts`：院子里的机器人；
  - `apps/desktop/src/components/studio/town/TownView.tsx`。

## 背景

前端已经做好了「项目小镇」（路由 `/town`）：一个项目一栋房子。

- 房子造型由项目类型决定，有 8 种：personal、team、experiment、client、backend、data、docs、ops。
- 大小分三档，由规模决定。
- 院子里机器人的数量 = 这个项目分配的 Agent 数。

现在有三个缺口：

1. 项目类型、屋顶颜色、门牌样式、院子摆件只能靠前端按项目名去猜，没地方保存。
2. 「项目分配了哪些 Agent」没有正式数据。现在的 `ProjectSummary.agents` 是从近 30 天的任务里推出来的。用户要的是**自己指定几个就是几个**。
3. Agent 档案（自定义名字、配色 / 配件、口头禅）没有地方存，要求换设备不丢。

## 要做的事

### 1. 项目外观 `ProjectAppearance`

合同放在 `crates/contracts`，需要 specta + serde camelCase + `#[serde(default)]`：

```rust
pub struct ProjectAppearance {
    pub project_id: String,
    /// None = 用户没选，前端按项目名猜。存未知字符串也要原样保存（前端会降级成民房），不要在后端校验成枚举。
    pub kind: Option<String>,
    /// 屋顶主色，限制为 `#rrggbb`，其他格式拒绝。
    pub roof: Option<String>,
    /// "wood" | "neon" | "stone"
    pub sign: Option<String>,
    /// 院子摆件："fountain" | "flag" | "statue" | "garden" | "bench" | "pet"，去重，最多 6 个。
    pub yard: Vec<String>,
    pub updated_at: Option<String>,
}
```

- 迁移：新表 `project_appearance`，`project_id` 是主键，外键指向 projects，删除项目时级联删除。
- 命令：
  - `project_appearance_list() -> Vec<ProjectAppearance>`：每个项目一行，没存过的也返回默认值，和 `project_summaries` 一样。
  - `project_appearance_update(projectId, patch)`：走 daemon，返回更新后的值。patch 里字段缺省表示不改；显式传 null 表示清空。
- 校验：
  - `sign` 和 `yard` 只接受上面列出的值；
  - `kind` 长度不超过 32，只允许 `[a-z0-9_-]`；
  - 校验失败返回现有的错误类型，并给出中文提示。

### 2. 项目成员（分配的 Agent）`ProjectMember`

```rust
pub struct ProjectMember {
    pub agent: AgentKind,
    /// "developer" | "reviewer" | "both"
    pub role: String,
    /// 排序，小的在前；前端按这个顺序摆机器人。
    pub position: i32,
    pub added_at: String,
}
```

- 迁移：新表 `project_members`。
  - 主键是 `(project_id, agent)`，`agent` 用 `AgentKind::as_str()` 的字符串存。
  - 外键级联删除。
- 命令：
  - `project_members_get(projectId) -> Vec<ProjectMember>`。
  - `project_members_set(projectId, members: Vec<{agent, role}>)`：整组替换，走 daemon；`position` 按数组顺序重排。
  - 同一个 agent 重复出现时拒绝；一个项目最多 12 个成员。
- **改 `ProjectSummary`**：
  - 新增 `assigned_agents: Vec<AgentKind>`，按 `position` 排序。
  - 保留现有 `agents`，即近 30 天任务推导出的结果，不改它的含义，以免影响别的地方。
  - 新增 `agents_source: "assigned" | "derived"`：项目有成员时是 assigned，没有时是 derived。
  - 更新 `project_summary.rs` 的纯函数和测试。
- 兼容：老项目没有成员记录。**不要**在迁移里自动回填，保持 derived，由用户在前端分配。

### 3. Agent 档案 `AgentProfile`

全局的，不分项目。一个 `AgentKind` 一份档案：

```rust
pub struct AgentProfile {
    pub agent: AgentKind,
    /// 显示名，1~16 个字符（按字符数算，中文算 1），去掉首尾空白；None = 用默认名。
    pub nickname: Option<String>,
    /// 机身配色，`#rrggbb`。
    pub color: Option<String>,
    /// 配件："none" | "cap" | "headphones" | "glasses" | "bowtie" | "antenna" | "scarf"
    pub accessory: Option<String>,
    /// 口头禅，最多 40 个字符。
    pub catchphrase: Option<String>,
    pub updated_at: Option<String>,
}
```

- 迁移：新表 `agent_profiles`，主键是 `agent`。
- 命令：
  - `agent_profiles_list() -> Vec<AgentProfile>`：只返回存过的档案。
  - `agent_profile_update(agent, patch)`：走 daemon，patch 语义同上。
  - `agent_profile_reset(agent)`：走 daemon，删除这一行。
- 这次的决定：档案按 `AgentKind` 存，不和 Provider 席位绑定。做完后把这条写进方案文档的「尚未决定」一节，并标为已定。

### 4. 桌面接线

- 在 `apps/desktop/src-tauri/src/main.rs` 的 `collect_commands` 里注册所有新命令。
- 然后跑 `cargo run -q -p xtask`，重新生成 `apps/desktop/src/generated/bindings.ts`。**不要手改这个文件。**
- 前端 hooks 放在 `src/hooks/useProjects.ts`，或新建一个 `src/hooks/useAgentProfiles.ts`：
  - 用 TanStack Query；
  - 在 `src/lib/queryKeys.ts` 加 key；
  - 写操作成功后，让 `projectSummaries`、appearance、members 和 profiles 的相关缓存失效。
- 浏览器预览 shim `src/lib/tauriDevShim.ts`：
  - 给新命令加上内存实现：读写都存在一个模块级 Map 里，刷新页面就没了，这样就够了；
  - 给 `DEMO_TOWN` 的几个演示项目预置一些外观和成员，让小镇有的房子是用户「选过」的样子；
  - 带 `visual` 参数时不返回演示数据，保持视觉基线不变。
  - shim 已经接近 500 行，超过就把新增部分拆到 `src/lib/devShim/studio.ts` 再导入。
- 小镇接上真数据：
  - `TownView.tsx` 里 `buildingFor(p.name, undefined, …)` 的 `undefined` 换成真实外观；
  - `townResidents.ts` 优先用 `assignedAgents`，为空时退回 `agents`；
  - 规模档位 `tierFor` 的 agents 数也按同样规则取。
  - **不要**做设置界面（选类型、选颜色、分配成员的表单），那是下一个任务。只把数据接通。

## 约束

- Rust：
  - 新文件用 `rustfmt --edition 2024` 格式化；
  - `cargo clippy -p agentflow-orchestrator -p agentflow-persistence -p agentflow-contracts` 不能有新警告；
  - 不改 `src-tauri/src/error.rs` 里已有的格式问题。
- 查询用 sqlx，和现有代码一致。迁移只能新增，不能改旧迁移。
- 前端：
  - 单个文件不超过 500 行；
  - 样式用 Tailwind，不新增 CSS 文件；
  - 字面量颜色只能写在 `src/styles/theme.css`，`styleDebt` 测试会检查。
- 所有文案用简体中文，口语化，与现有界面一致。

## 测试（全部要过）

- 每张新表在 orchestrator 里都要有用真实 SQLite 的读写测试，可以照 `project_summary.rs` 里 `Orchestrator::open` 的用例写。至少覆盖：
  - 默认值；
  - patch 部分更新；
  - 显式 null 清空；
  - 非法值被拒；
  - 删项目后级联删除；
  - `External("my-sidecar")` 类型的 agent 能存能读；
  - members 整组替换后 `position` 重排；
  - `ProjectSummary.assignedAgents` 和 `agentsSource` 的两种情况。
- daemon 的写路径至少各有一个测试，可以照 `ProjectSettingsUpdate` 现有的测试写。
- 命令：
  - `cargo test -p agentflow-orchestrator -p agentflow-persistence -p agentflow-contracts`
  - `cargo check -p agentflow-desktop`（以 `apps/desktop/src-tauri/Cargo.toml` 里的实际包名为准）
  - 在 `apps/desktop` 下：`bun test`、`bunx tsc --noEmit -p .`、`bun run test:visual`（基线应该**不变**，如果变了，说明 visual 模式漏了判断）、`bun run build`。

## 收尾

- 在 `docs/像素小屋_落地方案_2026-11-06.md` 末尾加一节「项目外观、成员与 Agent 档案接口」，写明：
  - 表结构；
  - 命令列表；
  - patch 语义；
  - 校验规则；
  - `agentsSource` 的回退规则。
- 最后报告：
  - 新增和改动的文件；
  - 每条测试命令的结果；
  - 以及任何没做完或做了取舍的地方。
