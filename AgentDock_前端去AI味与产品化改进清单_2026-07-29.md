# AgentDock 前端去 AI 味与产品化改进清单

- 日期：2026-07-29
- 对象：AgentFlow macOS 桌面端
- 当前版本：`0.1.0`
- 实机检查：`/Applications/AgentFlow.app`
- 检查页面：项目首页、需要你处理中心、任务详情、结果与验收、设置、Provider 与执行节点、新建任务
- 本轮边界：只重构信息架构、视觉语言和交互表达，不重建已经稳定的 Rust/Tauri/Provider 协议

## 结论

现在应该进入前端产品化阶段。

当前界面已经不是“不能用”，核心状态也基本真实；主要问题是功能增长后，每个模块都通过卡片、胶囊、图标、色块和说明文字争夺注意力。页面看起来完整，但不像一个长期打磨的桌面工具，更像把多个 AI 生成的优秀局部拼在了一起。

改版目标不是“更炫”，而是：

1. 用户打开页面后 3 秒内知道现在发生了什么、自己要不要行动。
2. 一个页面只有一个视觉主角，其余内容按需展开。
3. 状态只表达事实，不用光晕、渐变和持续动画制造“智能感”。
4. 让 AgentFlow 像一个可靠的多 Agent 调度台，而不是聊天机器人、营销落地页或普通 SaaS 后台。
5. 保留 AgentFlow 自己的辨识度：多 Agent 执行轨道、人工检查点、证据链和安全边界。

## 一、什么是当前页面的“AI 味”

### 1. 装饰性智能感过多

实机首页同时出现问候语、Sparkles 图标、暖色光晕、扫光动画、数字入场动画、卡片悬浮和脉冲圆点。单个效果没有问题，但组合在一起会接近常见 AI 网站模板。

代码侧可见：

- `ProjectOverview.tsx` 使用问候语、Sparkles、双径向光晕、sheen 扫光、KPI 数字动画和 hover 浮起。
- `theme.css` 同时定义 glass、gradient、glow、shimmer、sheen、pulse-dot。
- 当前 TSX 中约有 24 个 Motion 元素、58 处 `rounded-full`、44 处 panel 圆角容器、9 处旋转加载动画。

### 2. 卡片套卡片，所有内容都像一个“功能模块”

首页是 Hero 卡片 + 4 张 KPI 卡 + “需要你处理”大卡片 + 13 张任务卡。设置页是分区卡片、状态卡片、Provider 卡片和节点卡片继续嵌套。任务详情又叠加执行树、控制台、标签页、验收卡片和底部行动条。

结果是：

- 边框和圆角失去层级意义。
- 用户需要逐块阅读，无法快速扫描。
- 页面纵向很长，但有效决策信息密度并不高。
- 每个区块都像主角，真正的任务状态反而不突出。

### 3. 页面在重复解释，而不是帮助决策

首页任务行同时出现“需要你处理”“查看恢复方式”“下一步：查看失败原因，并选择重试、修复或取消”。任务详情又重复状态、原因、验收缺口和操作提示。

这些文字在语义上正确，但重复之后会产生明显的机器生成感：完整、礼貌、面面俱到，却不像成熟工具的短促操作语言。

### 4. 状态色和品牌色职责混在一起

当前 `human` 与 `run` 使用同一组陶土橙。暖橙既是品牌气氛、主按钮、运行状态，又是人工介入和警告色。用户只能靠文字分辨语义。

另外首页视觉明显接近 Claude 的暖色系，但 AgentFlow 是多 Provider 产品，不应该让任何一个 Provider 的品牌气质主导整个应用。

### 5. 动效没有严格区分“状态变化”和“装饰”

持续 pulse、shimmer、sheen、spin 与 hover 位移同时存在。真正有价值的运行状态动画因此不再稀缺，用户也更难区分“系统正在工作”和“页面只是看起来有生命”。

### 6. 桌面工具感不足

当前首页更像欢迎仪表盘，新建任务更像一张较长的 SaaS 表单。对于高频桌面工作流，更合适的是紧凑工具栏、可扫描列表、固定上下文、键盘优先和渐进式高级设置。

## 二、建议的视觉方向

### 推荐：安静的技术工作台

关键词：`calm`、`precise`、`operational`、`native`、`evidence-first`。

- 保留浅色主题作为第一阶段，避免视觉重构与暗色适配同时扩大范围。
- 主背景改为更中性的暖灰白，取消全局径向光晕。
- 大多数面板依靠间距、分隔线和背景层级，不依赖阴影与大圆角。
- 人工介入保留陶土橙；运行中改为冷蓝；成功为克制绿；失败为红；暂停/未知为中性灰。
- Provider 品牌色只用于头像、轨道标记和身份，不用于大面积页面背景。
- 图标只用于识别动作或状态，不给普通标题配装饰图标。
- 动画只表达真实变化：开始、等待、完成、失败、分支汇合。
- 中文正文优先使用 macOS 系统字体；任务编号、commit、路径、时间和命令使用等宽字体。

### 不建议继续的方向

- 不继续增加玻璃、渐变、霓虹、光球和扫光。
- 不用大面积聊天气泡把 Agent 运行伪装成聊天产品。
- 不照搬 Linear、Raycast、GitHub Desktop；学习它们的密度和克制，但保留 AgentFlow 的执行树与证据链。
- 不为了“高级感”全面改成纯黑界面。
- 不让每一个 Provider 都把品牌色扩散到整张页面。

## 三、P0：先消除最明显的 AI 模板感

### [x] UI-P0-01 移除首页营销式 Hero

**现状**

首页顶部使用问候语、Sparkles、渐变背景、双光晕、扫光和入场动画；它占据最大视觉面积，却不承载核心任务决策。

**修改**

- 删除“晚上好，欢迎回来”和 Sparkles。
- Hero 改为 56–64px 高的项目工具栏：项目名、分支、执行策略、环境状态、导出、新建任务。
- 移除双径向光晕、sheen 扫光和 hover 缩放。
- 项目名是唯一一级标题；环境状态降为工具栏中的紧凑状态入口。
- “新建任务”保留为唯一实心主按钮。

**主要文件**

- `apps/desktop/src/components/home/ProjectOverview.tsx`
- `apps/desktop/src/styles/theme.css`

**验收**

- 首页首屏不再出现问候语、Sparkles、扫光或背景光球。
- 1024px 宽窗口中，项目上下文和主操作保持在一行或可预测地换为两行。
- `prefers-reduced-motion` 与普通模式下信息完全一致。

### [x] UI-P0-02 把四张 KPI 卡改成一条状态摘要

**现状**

“全部任务 / 需要你 / 进行中 / 已完结”四个数字分别占一张带阴影卡片，视觉成本大于信息价值。

**修改**

- 改成工具栏下方的一行 segmented summary，或直接并入任务列表筛选栏。
- 默认选中“需要你”，数字只作为附属计数。
- “进行中”仅在确有活动时显示一个微弱动态标记。
- 删除数字滚动、交错入场和卡片 hover 上浮。

**主要文件**

- `apps/desktop/src/components/home/ProjectOverview.tsx`
- `apps/desktop/src/components/home/AnimatedNumber.tsx`
- `apps/desktop/src/routes/TaskList.tsx`

**验收**

- 4 个指标的总高度不超过 44px。
- 计数变化不会导致布局跳动。
- 用户可直接用该摘要筛选任务，而不仅是查看数字。

### [x] UI-P0-03 把“需要你处理”改为决策队列，不再一任务一卡片

**现状**

13 个待处理任务全部展开成卡片，每行重复状态、原因、下一步和行动文字，首屏只能看到少量条目。

**修改**

- 使用紧凑的列表/表格结构：优先级标记、任务、阻断原因、等待时长、唯一主操作。
- 默认按严重度和等待时长排序，而不是只按更新时间。
- 相同原因连续出现时允许分组，例如“4 个任务等待计划批准”。
- 原因正文限制一行，下一步通过主操作按钮体现；详细说明进入任务后再展示。
- 首屏最多展示 6–8 条，提供“查看全部 13 条”，不要无限向下铺开。
- 图标底色只给真正高风险项，普通待审批不使用警告三角形。

**主要文件**

- `apps/desktop/src/components/home/AttentionCenter.tsx`
- `apps/desktop/src/lib/attention.ts`
- `apps/desktop/src/routes/TaskList.tsx`

**验收**

- 1152×768 窗口首屏至少可见 6 条待处理事项。
- 每行只出现一个状态标签、一个原因和一个主要动作。
- 不再重复“需要你处理”“查看恢复方式”“下一步”三层同义信息。
- 键盘上下键可移动选择，Enter 打开，焦点状态清晰。

### [x] UI-P0-04 重建状态色语义

**现状**

品牌橙、运行中、人工介入和警告共用相近色值。

**修改**

建立不可混用的语义色：

| 语义 | 建议 | 使用范围 |
| --- | --- | --- |
| 人工介入 | 陶土橙 | 等待批准、权限请求、需要选择 |
| 运行中 | 冷蓝 | 真实活跃进程、当前执行轨道 |
| 成功 | 深绿 | 已验证、已合并、可交付 |
| 失败 | 克制红 | 运行失败、验证失败、拒绝 |
| 审查 | 紫灰 | 独立审查阶段，不代表失败 |
| 未开始/未知 | 中性灰 | 无结果、未检测、未配置 |

- 品牌主色不等于状态色。
- 大面积背景只使用中性色；状态色用于 2–4px 标记、图标、文字或浅色 wash。
- 所有状态仍必须用文字/图标表达，不能只靠颜色。

**主要文件**

- `apps/desktop/src/styles/theme.css`
- `apps/desktop/src/components/StateBadge.tsx`
- `apps/desktop/src/components/execution/StateMark.tsx`
- 所有 `text-human` / `bg-human-bg` / `text-run` 使用点

**验收**

- “正在运行”和“等你批准”在不读文字时也能明显区分。
- 现有 `styles/contrast.test.ts` 全部通过。
- 新增状态令牌快照测试，禁止 `human` 与 `run` 再次指向同一颜色。

### [x] UI-P0-05 取消无意义的圆角、阴影和胶囊

**现状**

圆角和胶囊几乎覆盖所有容器，导致层级扁平化。

**修改**

- 应用外壳和主内容区不使用卡片圆角。
- 一级浮层/对话框：10px；普通区块：6px；输入框/按钮：6px；列表行：4–6px。
- 胶囊只用于短状态或筛选，不用于普通说明文字。
- 默认面板不加阴影；阴影只给浮层、菜单、拖拽对象和临时抬升内容。
- 删除 `panel-raise`、`shadow-glow-*` 的常规页面使用。

**主要文件**

- `apps/desktop/src/styles/theme.css`
- `apps/desktop/src/components/ui/*`
- `apps/desktop/src/routes/TaskList.tsx`
- `apps/desktop/src/routes/detail/*`
- `apps/desktop/src/routes/settings/*`

**验收**

- 同一视口中最多只有两级带边框表面。
- 普通列表行不再 hover 上浮。
- `rounded-full` 只剩状态点、头像和真正的 pill 控件。

### [x] UI-P0-06 精简全局动效

**修改**

- 删除背景 sheen、装饰 shimmer、数字滚动、普通卡片 hover 位移。
- pulse 只允许用于有真实进程心跳的“运行中”，等待人工不能 pulse。
- spin 只允许用于 400ms 以上仍未完成且无法展示阶段进度的操作。
- 页面切换只做 120–160ms opacity；不使用 scale、弹簧或大距离位移。
- 弹窗只淡入并轻微上移，不让背景页面产生 3D/缩放感。
- 新增动效使用表，记录触发条件、停止条件与 reduced-motion 行为。

**主要文件**

- `apps/desktop/src/styles/theme.css`
- `apps/desktop/src/components/Dialog.tsx`
- `apps/desktop/src/components/home/*`
- `apps/desktop/src/routes/TaskList.tsx`
- `apps/desktop/src/components/execution/*`

**验收**

- 静止页面没有任何循环装饰动画。
- 任务进入终态后，所有运行指示在一次状态刷新内停止。
- `prefers-reduced-motion: reduce` 下无位移、缩放和循环动画。

### [x] UI-P0-07 建立统一排版系统

**修改**

- 中文界面改为 `-apple-system, BlinkMacSystemFont, "PingFang SC"` 优先；IBM Plex Sans 只作为非 macOS 回退，避免中英文混排气质割裂。
- 固定 5 个文字角色：页面标题、区块标题、正文、辅助信息、代码/标识。
- 停止在组件内散落 `text-[11px]`、`text-[12px]`、`text-[13px]`、`text-[14px]`。
- TASK 编号、revision、commit、路径、命令使用 mono；普通时间和计数使用 tabular numbers，不全部使用 mono。
- 中文正文行高不少于 1.55；说明文字单段尽量不超过两行。

**主要文件**

- `apps/desktop/src/styles/theme.css`
- `apps/desktop/src/components/ui/*`

**验收**

- 组件源码不再新增任意像素字号。
- 200% 缩放下标题、正文和辅助文字仍保持清楚层级。
- 中英文、数字、路径混排时基线稳定。

### [x] UI-P0-08 建立页面级视觉回归基线

**修改**

为以下状态建立固定数据夹具与截图基线：

- 空项目
- 多任务首页
- 需要人工处理 13 条
- 单任务运行中
- 多 Agent 并行审查
- Provider 降级
- 权限请求
- 验证失败
- 已完成等待最终批准
- 已合并
- 设置页完整状态
- 新建任务基础与高级模式

建议使用 Playwright component/e2e screenshot；真实 Tauri 安装版继续做最终人工检查，不能被快照替代。

**主要文件**

- `apps/desktop/src/**/__visual__/*`（建议新增）
- `apps/desktop/playwright.config.ts`（建议新增）
- `.github/workflows/ci.yml`

**验收**

- 1280×800、1024×768、800×600、200% 缩放均有基线。
- CI 能发现布局溢出、状态动画未停止和意外主题漂移。
- 动态时间、运行耗时和随机 ID 在截图夹具中固定。

## 四、P1：重做三个核心工作区

### [x] UI-P1-01 首页改为“队列优先”的工作台

**建议骨架**

```text
项目名 / main / 环境正常                         导出   新建任务
需要你 13   运行中 1   已完成 2
────────────────────────────────────────────────────────────
需要你处理
  优先级  TASK      原因                    等待      操作
  高      015       未产生改动              20 分钟   继续
  普通    014       质量门禁未通过          4 小时    查看
  ……
────────────────────────────────────────────────────────────
运行中
  TASK-016  Qoder 开发  02:18  [查看]
────────────────────────────────────────────────────────────
最近完成                                             查看全部
```

**要点**

- 首页不是统计仪表盘，而是工作入口。
- 删除“已完结 2”空标题单独占据大面积空间的问题。
- 历史任务默认只显示最近 5 条，其余进入筛选后的任务列表。
- 页面保留一个统一搜索/筛选入口，支持 TASK 编号、标题、Provider、状态。

**主要文件**

- `apps/desktop/src/routes/TaskList.tsx`
- `apps/desktop/src/components/home/ProjectOverview.tsx`
- `apps/desktop/src/components/home/AttentionCenter.tsx`
- `apps/desktop/src/components/home/HomeEmpty.tsx`

### [x] UI-P1-02 任务详情围绕“当前事实”重排

**现状**

任务详情顶部同时展示标题、状态、Agent、revision、预算、交付；下方又出现执行树、多 Agent 控制台、标签页、结果摘要、验收、证据、原始目标、版本记录和底部行动条。信息都重要，但没有明确主次。

**建议骨架**

```text
← TASK-015  标题                         当前状态          主要操作
  Qoder → Grok · r3 · 本机验证 · 本地合并
────────────────────────────────────────────────────────────
执行轨道（固定上下文，紧凑一行或可展开）
计划 ✓ ─ 开发 ✓ ─ 验证 — ─ 审查 — ─ 批准 ! ─ 交付 —
────────────────────────────────────────────────────────────
结果 | Diff | 审查 | 日志 | 治理

当前结论
这一轮没有产生改动
原因……                                      补充说明并继续

验收证据（默认展开）
历史与系统详情（默认折叠）
```

**修改**

- 把执行树与多 Agent 控制台合并为一个“执行轨道”概念：阶段为主轴，Provider 为分支。
- 当前结论只出现一次；底部粘性行动条只保留动作，不再复制整段原因。
- 结果页优先显示证据和未闭环项；原始目标、版本历史进入折叠区。
- “系统详情”保持技术层，不参与首屏竞争。
- 宽屏允许右侧 Inspector 展示当前节点细节；窄屏改为抽屉。

**主要文件**

- `apps/desktop/src/routes/TaskDetail.tsx`
- `apps/desktop/src/routes/detail/OverviewTab.tsx`
- `apps/desktop/src/components/execution/ExecutionTree.tsx`
- `apps/desktop/src/components/execution/AgentConsole.tsx`
- `apps/desktop/src/components/execution/LiveStatusBar.tsx`
- `apps/desktop/src/components/ApprovalBar.tsx`

**验收**

- 用户无需滚动即可看到当前状态、原因、下一步和核心证据。
- 同一句失败原因不在首屏重复超过一次。
- 已合并任务没有任何看似可执行或仍在运行的视觉元素。
- 并行、串行、降级和结构修复在轨道上有不同几何关系，不靠长句解释。

### [x] UI-P1-03 新建任务改为“基础创建 + 高级控制”

**现状**

单个对话框同时展示标题、验收条件、描述、开发/审查 Agent、目标分支、返工轮数、计划门禁、交付、执行位置、优先级、Token、费用、时间和质量阈值。初次创建任务时认知负担过高。

**修改**

- 基础区默认只展示：标题、描述、验收条件、开发 Agent、审查 Agent。
- 把分支、返工、计划门禁、交付、执行位置、优先级和预算收入“运行与治理”折叠区。
- 折叠标题直接回显摘要，例如“main · 本机 · 最多 3 轮 · $25”。
- 将“创建并立即开始”设为主按钮，“仅创建草稿”改为次级文本动作。
- Provider 下拉直接展示可用性，但不在选项中塞入长篇诊断；详细问题链接到设置。
- 对话框宽度根据窗口调整：宽屏 680–760px，窄屏全高 sheet；避免 480px 窄列承载复杂表单。
- 验收条件支持键盘快速添加，类型选择和内容输入保持在同一行。

**主要文件**

- `apps/desktop/src/routes/NewTaskDialog.tsx`
- `apps/desktop/src/components/Dialog.tsx`
- `apps/desktop/src/components/ui/select.tsx`

**验收**

- 使用默认策略创建普通任务只需填写标题、描述并确认 Agent。
- 高级设置折叠时仍可看见最终策略摘要。
- 800×600 与 200% 缩放下 footer 始终可访问，内容区独立滚动。
- `Cmd+Enter` 行为明确，不能绕过校验或 Preflight。

### [x] UI-P1-04 设置页增加左侧分区导航

**现状**

基础环境、Provider、执行节点、运行、通知、存储与隐私全部放在一个长页面中。检测结果、配置和危险操作混在同一滚动流。

**修改**

- 设置内容区增加二级导航：环境、Provider、执行、权限、安全与存储、通知。
- 默认进入“环境概览”，只展示总状态与需要修复的项。
- 详细系统信息默认折叠；不要在首屏直接展示钥匙串完整路径等诊断内容。
- Provider 使用统一行式列表；认证、配置、版本和探针结果进入右侧详情面板。
- 危险操作单独放入底部“数据与恢复”，使用明确边界，不与普通开关混排。
- 保存按钮只在对应分区有未保存变更时出现，并固定在分区底部。

**主要文件**

- `apps/desktop/src/routes/Settings.tsx`
- `apps/desktop/src/routes/settings/EnvSection.tsx`
- `apps/desktop/src/routes/settings/ProviderSection.tsx`
- `apps/desktop/src/routes/settings/ExecutionNodeSection.tsx`
- `apps/desktop/src/routes/settings/StorageSection.tsx`

**验收**

- 进入设置后 3 秒内可判断端到端环境是否可运行。
- 用户不滚动即可到达任意设置分区。
- 配置错误定位到具体字段，不只在区块顶部报错。

### [x] UI-P1-05 统一状态与动作短文案

建立三层文案，不允许互相重复：

1. **状态**：发生了什么，例如“等待计划批准”。
2. **原因**：为什么停住，例如“计划已生成，尚未获得你的批准”。
3. **动作**：下一步做什么，例如“审阅计划”。

规则：

- 按钮使用动词，不使用“查看并决定”“查看恢复方式”这类泛化词。
- 不在原因后再次拼接“下一步：”。
- “需要你处理”只作为页面/队列分类，不重复成为每行 badge。
- “尚无结果”只在结果确实尚未产生时使用；若阶段未开始，直接写“未开始”。
- 技术词第一次出现时解释一次，后续使用稳定短称。
- 禁止装饰性符号和拟人化祝贺，例如“✦ 一切顺利”。

**主要文件**

- `apps/desktop/src/copy/*`
- `apps/desktop/src/lib/attention.ts`
- `apps/desktop/src/lib/taskResult.ts`
- `apps/desktop/src/lib/execution/*`

### [x] UI-P1-06 建立稀缺的主操作规则

- 每个页面最多一个实心主按钮。
- 每个卡片/列表行最多一个直接可见主动作。
- 取消、删除、强制批准不得和继续/批准具有相同视觉权重。
- 同一动作不得同时出现在标题区、内容区和底部行动条。
- 不可用按钮必须说明阻断条件，但说明不应常驻占据主界面。
- 命令型动作尽量支持快捷键，并在 tooltip 或菜单中显示。

**主要文件**

- `apps/desktop/src/components/ui/button.tsx`
- `apps/desktop/src/components/ApprovalBar.tsx`
- `apps/desktop/src/components/PlanApprovalBar.tsx`
- `apps/desktop/src/routes/NewTaskDialog.tsx`

### [x] UI-P1-07 让 Provider 身份成为 AgentFlow 的独特视觉资产

- 统一 Provider mark：固定尺寸、形状和边框，品牌色只占小面积。
- 执行轨道中用 Provider mark + 名称表示谁在执行，不重复写“开发 Agent / Qoder CLI”。
- fallback 用纵向尝试序列；并行审查用并排分支；同 Provider 结构修复用同一轨道的 retry 标记。
- 不用彩色大卡片区分 Provider。
- 未安装、未认证、不兼容分别使用一致的小状态，不混成一个红点。

**主要文件**

- `apps/desktop/src/components/ProviderIcon.tsx`
- `apps/desktop/src/components/AgentMark.tsx`
- `apps/desktop/src/components/execution/TreeNodes.tsx`
- `apps/desktop/src/components/ProviderCatalog.tsx`

### [x] UI-P1-08 统一空状态、加载、失败和只读状态

- 空状态不使用插画、Sparkles 或营销式口号；说明当前事实和唯一下一步。
- 骨架屏只模拟真实最终布局，不用通用灰条填满页面。
- 加载超过 400ms 才显示；能显示阶段时不用无限 spinner。
- 失败页面必须保留已完成内容，局部失败不替换整页。
- 历史任务与已合并任务使用只读视觉，不出现 hover 主操作暗示。

**主要文件**

- `apps/desktop/src/components/ErrorState.tsx`
- `apps/desktop/src/components/Skeleton.tsx`
- `apps/desktop/src/components/home/HomeEmpty.tsx`
- `apps/desktop/src/components/execution/RunFailureDetail.tsx`

## 五、P2：形成长期可维护的设计系统

### [x] UI-P2-01 建立语义令牌而不是页面私有颜色

- Surface：`canvas / sidebar / panel / elevated / overlay`
- Text：`primary / secondary / muted / inverse / link`
- Border：`subtle / default / strong / focus`
- Status：`running / human / success / danger / review / idle`
- Space：4、8、12、16、24、32
- Radius：4、6、10
- Motion：120ms、160ms、220ms，只保留 2 条 easing

删除组件中的任意 hex、任意 shadow、任意 radius 和重复渐变。

### [x] UI-P2-02 建立基础组件清单与使用边界

至少统一：

- `PageHeader`
- `SectionHeader`
- `Toolbar`
- `StatusBadge`
- `ProviderMark`
- `DataRow`
- `ActionRow`
- `EmptyState`
- `InlineNotice`
- `Inspector`
- `Dialog / Sheet / Popover`
- `Tabs / SegmentedControl`

每个组件写清：适用场景、禁止场景、尺寸、状态、键盘行为和无障碍名称。

### [x] UI-P2-03 增加密度模式，但不增加主题复杂度

- 默认使用舒适密度。
- 任务列表、日志和执行轨道允许切换紧凑密度。
- 密度改变只影响行高和间距，不改变信息、颜色和交互。
- 暗色主题在浅色系统稳定后再做，不与本轮混在一起。

### [x] UI-P2-04 完善 macOS 桌面细节

- 顶部留出明确的 traffic-light 安全区。
- 支持窗口缩放、全屏、侧栏折叠和系统 reduced motion。
- 原生快捷键：`⌘N` 新建、`⌘K` 搜索/命令、`⌘,` 设置、`⌘[` 返回。
- 文本选择、右键菜单、复制 commit/路径和打开文件位置保持原生预期。
- 不自定义滚动条到明显偏离 macOS；只做轻量配色。
- 弹窗焦点恢复、Select portal 和 Escape 行为继续保留现有测试。

### [x] UI-P2-05 建立“视觉债务”CI 规则

可逐步加入静态门禁：

- 禁止新增任意 hex 色值。
- 禁止新增未登记的 `text-[Npx]`、`rounded-[Npx]`、`shadow-[...]`。
- 禁止没有 `motion-reduce` 策略的循环动画。
- 禁止一个组件出现两个以上实心主按钮。
- 对页面级组件设置建议行数，避免继续堆叠职责。
- PR 必须附受影响页面的前后截图和至少一个异常状态截图。

## 六、实施顺序

### 第一阶段：视觉减法（建议 2–3 个独立提交）

1. 重建状态色、排版、surface、radius、shadow 和 motion 令牌。
2. 首页移除 Hero/KPI 卡片装饰，改为工具栏 + 摘要。
3. “需要你处理”改为紧凑决策队列。

完成标志：不改变业务流程，首页已经明显摆脱 AI 模板感。

### 第二阶段：核心工作区（建议 3–4 个独立提交）

1. 合并执行树与多 Agent 控制台的重复表达。
2. 重排任务结果与验收页。
3. 新建任务分为基础与高级设置。
4. 设置页增加分区导航与渐进披露。

完成标志：用户在任务首页、任务详情和创建任务三个高频路径中不需要重复阅读说明。

### 第三阶段：系统化与验收

1. 提炼通用组件和文案规则。
2. 建立截图夹具和视觉回归测试。
3. 实机覆盖 4 种窗口尺寸、200% 缩放、键盘与 reduced motion。
4. 用 Qoder → Grok 再跑一条真实链路，检查运行、失败、审查、批准和合并终态。

完成标志：视觉质量可由规范和测试维持，而不是依赖某次人工美化。

## 七、实施边界

本轮前端改造可以读取并重组现有数据，但不要：

- 修改 Provider Protocol 1.2。
- 改写任务状态机或伪造新的状态。
- 用前端推测替代后端 `TaskStatus`、run、validation、review 和 delivery 事实。
- 为了页面好看隐藏高风险、失败或未知状态。
- 把未检测显示成成功，把安装等同于登录，把本地合并等同于远端交付。
- 删除现有权限、审计、恢复和可访问性能力。

如果某个设计需要后端新增字段，应单独列出契约需求并评审，不在组件内拼接猜测。

## 八、最终验收清单

### 视觉

- [x] 首屏只有一个主视觉焦点。
- [x] 静止页面没有循环装饰动画。
- [x] 普通页面没有三层以上卡片嵌套。
- [x] 运行、人工介入、成功、失败只看形态和颜色即可初步区分。
- [x] Provider 品牌色不形成大面积背景。
- [x] 页面中不再出现问候、星光、扫光和营销式欢迎文案。

### 信息

- [x] 当前状态、原因、下一步各出现一次且位置稳定。
- [x] 首页 3 秒内可判断最优先处理项。
- [x] 任务详情不滚动即可看到当前结论和主要操作。
- [x] 技术详情默认折叠，但始终可以到达。
- [x] 终态页面不残留运行动画或“准备中”。

### 交互

- [x] 800×600、1024×768、1280×800 和宽屏均无横向溢出。
- [x] 200% 缩放可完成创建、审批、恢复和查看结果。
- [x] 键盘可遍历任务队列、执行轨道、标签页和所有对话框。
- [x] reduced motion 下功能与状态信息不丢失。
- [x] 加载、空、失败、部分失败、只读、运行和终态都有独立表现。

### 真实性

- [x] 所有状态来自后端事实或明确标记为前端本地草稿。
- [x] 并行、串行、fallback、retry 和 review council 不互相伪装。
- [x] “通过”必须有验证/审查证据；“已交付”必须有对应交付事实。
- [x] 后到事件不能让已阻断或已合并任务重新显示运行状态。

## 九、完成记录（2026-07-30）

- P0、P1、P2 共 21 项已经实现并逐项勾选；改造只消费现有任务、运行、验证、审查、权限和交付事实，没有修改 Provider Protocol 1.2 或任务状态机。
- 桌面前端 `bun run typecheck` 通过，`bun test` 共 290 项通过、0 失败，生产构建通过。
- Playwright 建立 15 个场景、16 张 macOS 基线图，覆盖空项目、多任务、13 条人工队列、运行、并行审查、fallback、权限请求、验证失败、等待批准、已合并、设置、新建任务基础/高级、800×600、1024×768 和 200% 缩放；更新基线与逐像素比较均通过。
- `bun run app:build` 已产出并安装 `/Applications/AgentFlow.app`，同时生成 `AgentFlow_0.1.0_aarch64.dmg`。
- 真实安装版已操作验证：首页搜索和密度切换、新建任务基础/高级模式、Provider 可用性、设置六分区、任务执行轨道、结果与验收、执行详情抽屉均可到达且状态与本机数据一致。
- 实机验收发现并修复一个回归：开发/审查 Agent 曾被误收入“运行与治理”折叠区；现在两个 Agent 选择器始终位于基础区，高级参数仍保持折叠。
- 视觉债务门禁已接入测试与 CI；PR 模板要求前后截图和异常状态截图，Playwright 运行产物目录已加入 `.gitignore`。

## 十、原建议的最小切口（已完成）

先只改首页，不碰任务状态协议：

1. 删除 Hero 的问候、Sparkles、光晕和扫光。
2. 四张 KPI 卡改成一条可筛选摘要。
3. “需要你处理”改成高密度决策队列。
4. 分离 running 与 human 的颜色。
5. 删除普通列表的 hover 浮起和重复状态文案。
6. 对 13 条待处理、1 条运行中、2 条完成的当前真实数据做实机对比。

这个切口风险低、效果最明显，也能先确定整套设计语言，再扩展到任务详情、新建任务和设置页。
