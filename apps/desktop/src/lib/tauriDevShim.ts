/*
 * DEV-ONLY preview shim. When the frontend runs OUTSIDE the Tauri shell (e.g.
 * `bun run dev` opened in a browser), `window.__TAURI_INTERNALS__` is missing and
 * every generated command throws "Cannot read properties of undefined (reading
 * 'invoke')". This installs a stand-in that returns sample data so the UI can be
 * previewed in a browser.
 *
 * It activates ONLY when: import.meta.env.DEV AND there is no real Tauri host.
 * Inside the packaged Tauri app the real __TAURI_INTERNALS__ is present, so this
 * is inert and never bypasses the real backend (03 §1 / 02 "不 mock 上线").
 */
import type {
  DiffPayload,
  EnvReport,
  ExecutionNode,
  GitCompatibilityReport,
  GlobalSettings,
  OnboardingReport,
  PermissionDecision,
  PermissionDecisionInput,
  PermissionRequest,
  PermissionRule,
  Project,
  ProjectConfigTrust,
  ProjectSettings,
  ProviderDescriptor,
  QueueTaskState,
  Review,
  RunLogPage,
  RunSummary,
  StorageReport,
  TaskDetail,
  TaskEvent,
  TaskGovernance,
  TaskSummary,
  ToolStatus,
} from "@/generated/bindings";

const iso = (offsetMs: number) => new Date(Date.now() + offsetMs).toISOString();
const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

const okTool = (path: string, version: string, authed = true): ToolStatus => ({
  found: true,
  path,
  version,
  compatible: true,
  problem: null,
  authenticated: authed,
  authMethod: authed ? "account" : null,
  authProblem: authed ? null : "未登录，运行 login 后重试",
  supportLevel: "verified",
  verifiedVersions: [version],
});
const missingTool = (): ToolStatus => ({
  found: false,
  path: null,
  version: null,
  compatible: false,
  problem: "未找到可执行文件",
  authenticated: null,
  authMethod: null,
  authProblem: null,
  supportLevel: "untracked",
  verifiedVersions: [],
});

const ENV: EnvReport = {
  system: {
    os: "macos",
    osVersion: "macOS 15.5",
    architecture: "aarch64",
    agentflowVersion: "0.1.0",
    shell: "/bin/zsh",
    diskAvailableBytes: 128 * 1024 ** 3,
    network: { available: true, detail: "DNS 可用（api.github.com）", problem: null },
    keychain: { available: true, detail: "login.keychain-db", problem: null },
  },
  git: okTool("/usr/bin/git", "2.43.0"),
  node: okTool("/opt/homebrew/bin/node", "22.17.0"),
  bun: okTool("/opt/homebrew/bin/bun", "1.2.18"),
  claudeCode: okTool("/opt/homebrew/bin/claude", "1.0.0"),
  codex: okTool("/opt/homebrew/bin/codex", "0.9.2"),
  geminiCli: missingTool(),
  qwenCode: missingTool(),
  qoderCli: missingTool(),
  grokCli: missingTool(),
  kimiCli: missingTool(),
  minimaxCli: missingTool(),
  openaiApi: { configured: true, available: false, model: "gpt-4o", baseUrl: "https://api.openai.com/v1", keyEnv: "OPENAI_API_KEY", problem: "凭据不可用" },
  anthropicApi: { configured: true, available: true, model: "claude-opus-4-8", baseUrl: "https://api.anthropic.com", keyEnv: "ANTHROPIC_API_KEY", problem: null },
  deepseekApi: { configured: true, available: true, model: "deepseek-chat", baseUrl: "https://api.deepseek.com/v1", keyEnv: "DEEPSEEK_API_KEY", problem: null },
  grokApi: { configured: true, available: false, model: "grok-4.5", baseUrl: "https://api.x.ai/v1", keyEnv: "XAI_API_KEY", problem: "凭据不可用" },
  minimaxApi: { configured: true, available: false, model: "MiniMax-M2.7", baseUrl: "https://api.minimax.io/v1", keyEnv: "MINIMAX_API_KEY", problem: "凭据不可用" },
  kimiApi: { configured: true, available: false, model: "kimi-for-coding", baseUrl: "https://api.kimi.com/coding/v1", keyEnv: "KIMI_API_KEY", problem: "凭据不可用" },
};

const PROVIDERS: ProviderDescriptor[] = [
  ["claude_code", "Claude Code", true, true],
  ["codex", "Codex", true, true],
  ["gemini_cli", "Gemini CLI", true, false],
  ["qwen_code", "Qwen Code", true, false],
  ["qoder_cli", "Qoder CLI", true, false],
  ["grok_cli", "Grok CLI", true, false],
  ["openai_api", "OpenAI API", false, false],
  ["anthropic_api", "Anthropic API", false, true],
  ["deepseek_api", "DeepSeek API", false, true],
  ["grok_api", "Grok API", false, false],
  ["minimax_api", "MiniMax API", false, false],
  ["kimi_api", "Kimi API", false, false],
].map(([id, displayName, development, available]) => ({
  id: String(id),
  displayName: String(displayName),
  source: "builtin",
  protocolVersion: "1.0",
  capabilities: {
    development: Boolean(development), review: true, streaming: true,
    structuredOutput: true, sandbox: true, resume: Boolean(development),
  },
  executionLocation: String(id).endsWith("_api") ? "remote" : "local",
  dataEgress: String(id).endsWith("_api") ? "diff" : "none",
  permissions: {
    worktreeRead: !String(id).endsWith("_api"),
    worktreeWrite: Boolean(development),
    networkDomains: String(id).endsWith("_api") ? ["provider-api"] : [],
    commands: [],
  },
  trust: "builtin",
  available: Boolean(available),
  problem: available ? null : "当前未配置或未安装",
}));

const PROJECT: Project = {
  id: "p1",
  seq: 1,
  name: "acme-web",
  repoPath: "/Users/dev/acme-web",
  defaultBranch: "main",
  worktreeRoot: "/Users/dev/.agentflow/worktrees",
  createdAt: iso(-6 * DAY),
};

function summary(
  seq: number,
  title: string,
  status: TaskSummary["status"],
  blockedReason: TaskSummary["blockedReason"],
  rev: number,
  updated: number
): TaskSummary {
  return {
    id: `t${seq}`,
    projectId: "p1",
    seq,
    title,
    status,
    blockedReason,
    currentRevision: rev,
    developerAgent: "claude_code",
    reviewerAgent: "codex",
    updatedAt: iso(updated),
  };
}

const SUMMARIES: TaskSummary[] = [
  summary(12, "修复登录空指针", "WAITING_FOR_HUMAN_APPROVAL", null, 2, -2 * MIN),
  summary(9, "重构订单状态机", "BLOCKED", "permission_required", 3, -1 * HOUR),
  summary(14, "升级依赖到 React 18", "MERGE_CONFLICT", null, 1, -20 * MIN),
  summary(13, "给结算页加骨架屏", "DEVELOPING", null, 1, -10_000),
  summary(15, "补充搜索接口单测", "REVIEWING", null, 1, -3 * MIN),
  summary(7, "统一日期格式化工具", "MERGED", null, 2, -1 * DAY),
  summary(5, "移除废弃的 feature flag", "CANCELLED", null, 1, -3 * DAY),
];

const DEMO_POLICY = {
  requirePlanApproval: true,
  priority: 0,
  tokenBudget: 500_000,
  costBudgetUsd: 25,
  timeBudgetSecs: 7_200,
  minimumQualityScore: 70,
  deliveryMode: "local_merge" as const,
  executionNodeId: null,
};

/**
 * Preview budget deliberately exercises the "Token 可统计、费用未知" case from 05 §11:
 * the费用 column must read 未知 rather than $0, and a 已预留 segment must be visible.
 */
const DEMO_BUDGET = {
  tokensUsed: 362_000,
  costUsd: null,
  timeUsedSecs: 1_480,
  tokenBudget: 500_000,
  costBudgetUsd: 25,
  timeBudgetSecs: 7_200,
  tokensKnown: true,
  costKnown: false,
  unknownTokenRuns: 0,
  unknownCostRuns: 2,
  tokensReserved: 24_000,
  costReservedUsd: null,
  tokenEnforcement: "hard" as const,
  costEnforcement: "unavailable" as const,
  exceeded: false,
};

function detail(s: TaskSummary): TaskDetail {
  return {
    ...s,
    description:
      "线上登录接口在用户 profile 缺失头像字段时抛出空指针。\n\n期望：对缺失字段做兜底，补充相应单测，不改动其它登录逻辑。",
    targetBranch: "main",
    baseCommit: "a1b2c3d4e5f6",
    branch: `agentflow/TASK-${String(s.seq).padStart(3, "0")}`,
    maxRevisions: 3,
    acceptanceCriteria: [],
    blockedDetail:
      s.status === "BLOCKED"
        ? "AgentFlow 已先停止 Provider，等待你决定几项执行权限。"
        : s.status === "MERGE_CONFLICT"
          ? "主仓库 main 已前进，自动合并在 src/auth/login.ts 冲突。"
          : null,
    revisions: Array.from({ length: Math.max(1, s.currentRevision) }, (_, i) => ({
      revision: i + 1,
      commitSha: `9f8e7d6c5b4a${i}`,
      stat: { files: 3 + i, insertions: 120 + i * 40, deletions: 18 + i * 12, flagged: i === 1 ? ["CLAUDE.md"] : [] },
      createdAt: iso(-(s.currentRevision - i) * 12 * MIN),
    })),
    policy: DEMO_POLICY,
    plan: null,
    budget: DEMO_BUDGET,
    delivery: null,
  };
}

const DETAILS = new Map(SUMMARIES.map((s) => [s.id, detail(s)]));

/**
 * Preview events use the real backend `event_type` strings so the copy/adapter layer and
 * the execution tree are exercised for real. Covers the 05 §11 scenarios that are pure
 * frontend concerns: scheduler noise, two Provider fallbacks, a 3-member council with one
 * Provider failure, and a daemon reconnect that adopts a live run.
 */
function events(taskId: string): TaskEvent[] {
  const rows: Array<{
    type: string;
    actor: TaskEvent["actor"];
    revision: number | null;
    min: number;
    runId?: string;
    payload?: unknown;
  }> = [
    { type: "user:start", actor: "human", revision: 0, min: -62 },
    // 系统详情：不应出现在主执行树。
    { type: "scheduler:slot", actor: "orchestrator", revision: 1, min: -61, payload: { revision: 1, operation_id: "op-1" } },
    { type: "run:succeeded", actor: "agent", revision: 1, min: -50, runId: `run-${taskId}-1-developer-0`, payload: { commit_sha: "9f8e7d6c5b4a0", summary: "第 1 轮：修复空指针并补充单测。" } },
    { type: "review:request_changes", actor: "agent", revision: 1, min: -46 },

    { type: "scheduler:slot", actor: "orchestrator", revision: 2, min: -34, payload: { revision: 2, operation_id: "op-2" } },
    // 两次降级：必须折叠在同一个开发 run 下。
    { type: "provider:fallback", actor: "orchestrator", revision: 2, min: -31, payload: { role: "developer", from: "codex", to: "claude_code", reason: "配额不足" } },
    { type: "provider:fallback", actor: "orchestrator", revision: 2, min: -28, payload: { role: "developer", from: "claude_code", to: "deepseek_api", reason: "登录已过期" } },
    { type: "run:succeeded", actor: "agent", revision: 2, min: -22, runId: `run-${taskId}-2-developer-2`, payload: { commit_sha: "9f8e7d6c5b4a2", summary: "第 2 轮：按审查意见抽出常量并补充边界用例。" } },
    { type: "result:repair_succeeded", actor: "orchestrator", revision: 2, min: -21, payload: { role: "developer", agent: "deepseek_api", detail: "结构化结果缺少 changed_files，已补齐" } },

    // daemon 重连后接管仍在运行的 Agent：作为该 run 的子状态。
    { type: "recovery:run_adopted", actor: "system", revision: 2, min: -20, runId: `run-${taskId}-2-reviewer-0`, payload: { run_id: `run-${taskId}-2-reviewer-0`, pid: 4242, role: "reviewer" } },
    { type: "recovery:run_recovered", actor: "system", revision: 2, min: -19, runId: `run-${taskId}-2-reviewer-0`, payload: { run_id: `run-${taskId}-2-reviewer-0`, reused_result: true } },

    { type: "scheduler:council_slot", actor: "orchestrator", revision: 2, min: -18 },
    { type: "review:council_member_failed", actor: "orchestrator", revision: 2, min: -16, runId: `run-${taskId}-2-reviewer-2`, payload: { agent: "openai_api", error: "429 rate limited" } },
    { type: "integrity:security_review_required", actor: "orchestrator", revision: 2, min: -15, payload: { detail: "本轮触及控制面文件 CLAUDE.md" } },
    { type: "review:council_pass", actor: "agent", revision: 2, min: -14 },
  ];

  return rows.map((row, i) => ({
    id: i + 1,
    taskId,
    runId: row.runId ?? null,
    revision: row.revision,
    actor: row.actor,
    eventType: row.type,
    payload: row.payload ?? {},
    createdAt: iso(row.min * MIN),
  }));
}

function runs(taskId: string): RunSummary[] {
  const mk = (
    rev: number,
    role: RunSummary["role"],
    agent: RunSummary["agent"],
    status: RunSummary["status"],
    min: number,
    index = 0
  ): RunSummary => ({
    id: `run-${taskId}-${rev}-${role}-${index}`,
    taskId,
    revision: rev,
    role,
    agent,
    status,
    exitCode: status === "SUCCEEDED" ? 0 : status === "RUNNING" ? null : 1,
    // 费用未知的 run：预算卡必须显示「未知」而不是 $0。
    costUsd: null,
    tokensIn: 5_000,
    tokensOut: 1_800,
    startedAt: iso(min * MIN),
    finishedAt: status === "RUNNING" ? null : iso((min + 3) * MIN),
  });

  return [
    mk(1, "developer", "codex", "SUCCEEDED", -55),
    mk(1, "validator", null, "SUCCEEDED", -50),
    mk(1, "reviewer", "claude_code", "SUCCEEDED", -46),

    // 降级链：首选 → 降级 1 → 降级 2。
    mk(2, "developer", "codex", "FAILED", -34, 0),
    mk(2, "developer", "claude_code", "FAILED", -30, 1),
    mk(2, "developer", "deepseek_api", "SUCCEEDED", -26, 2),
    mk(2, "validator", null, "SUCCEEDED", -23),
    // 三人委员会：两人成功、一人 Provider 故障。
    mk(2, "reviewer", "claude_code", "SUCCEEDED", -18, 0),
    mk(2, "reviewer", "deepseek_api", "SUCCEEDED", -18, 1),
    mk(2, "reviewer", "openai_api", "FAILED", -18, 2),
  ];
}

const DIFF: DiffPayload = {
  baseCommit: "a1b2c3d4e5f6",
  commitSha: "9f8e7d6c5b4a2",
  diffSha256: "sha256:preview-mock-diff",
  truncated: false,
  files: [
    {
      path: "src/auth/login.ts",
      oldPath: null,
      binary: false,
      flagged: false,
      insertions: 9,
      deletions: 2,
      patch:
        "@@ -12,7 +12,9 @@ export function login(user: User) {\n" +
        "-  const avatar = user.profile.avatar;\n" +
        "-  return render(avatar);\n" +
        "+  const avatar = user.profile?.avatar ?? DEFAULT_AVATAR;\n" +
        "+  if (!user.profile) log.warn('profile missing', user.id);\n" +
        "+  return render(avatar);\n",
    },
    {
      path: "src/auth/login.test.ts",
      oldPath: null,
      binary: false,
      flagged: false,
      insertions: 24,
      deletions: 0,
      patch:
        "@@ -0,0 +1,6 @@\n" +
        "+test('falls back when profile missing', () => {\n" +
        "+  expect(login({ id: '1' })).toBeDefined();\n" +
        "+});\n",
    },
    {
      path: "CLAUDE.md",
      oldPath: null,
      binary: false,
      flagged: true,
      insertions: 1,
      deletions: 0,
      patch: "@@ -3,0 +4 @@\n+- 登录相关改动需补充单测\n",
    },
    // 测试删除必须被独立标记并阻挡批准 (05 §6.9 / §11)。
    {
      path: "src/auth/legacy-session.test.ts",
      oldPath: null,
      binary: false,
      flagged: false,
      insertions: 0,
      deletions: 42,
      patch: "@@ -1,42 +0,0 @@\n-test('legacy session still validates', () => {\n-  expect(validate(session)).toBe(true);\n-});\n",
    },
    {
      path: ".github/workflows/ci.yml",
      oldPath: null,
      binary: false,
      flagged: false,
      insertions: 2,
      deletions: 1,
      patch: "@@ -18,7 +18,8 @@ jobs:\n-      - run: bun test\n+      - run: bun test --bail\n+      - run: bun run typecheck\n",
    },
  ],
};

const REVIEW: Review = {
  id: "rev-1",
  revision: 2,
  commitSha: "9f8e7d6c5b4a2",
  decision: "request_changes",
  summary: "兜底逻辑合理，但删除历史 session 测试需要说明理由，控制面文件改动也要确认。",
  reviewerAgents: ["claude_code", "deepseek_api", "openai_api"],
  issues: [
    {
      id: "i0",
      severity: "high",
      file: "src/auth/legacy-session.test.ts",
      lineStart: null,
      lineEnd: null,
      title: "删除了历史 session 测试且没有等价替代",
      description: "本轮删除了 42 行测试，但新增的用例没有覆盖同样的分支。",
      suggestedAction: "恢复该测试，或说明为什么这条路径已不存在。",
      resolved: false,
      reportedBy: ["claude_code", "deepseek_api"],
      agreementCount: 2,
    },
    { id: "i1", severity: "low", file: "src/auth/login.ts", lineStart: 14, lineEnd: 14, title: "可考虑抽出 DEFAULT_AVATAR 常量", description: "多处用到默认头像，建议集中定义。", suggestedAction: "在 constants.ts 定义并复用。", resolved: false, reportedBy: ["claude_code"], agreementCount: 1 },
    { id: "i2", severity: "medium", file: "CLAUDE.md", lineStart: 4, lineEnd: 4, title: "修改了控制面文件", description: "本轮改动了 CLAUDE.md，请确认是否必要。", suggestedAction: "若非必要请回退。", resolved: false, reportedBy: ["claude_code", "deepseek_api"], agreementCount: 2 },
  ],
};

const SETTINGS: GlobalSettings = {
  maxConcurrentRuns: 2,
  schedulerPaused: false,
  runWindowStart: null,
  runWindowEnd: null,
  globalDailyCostUsd: null,
  defaultProviderMaxConcurrent: 1,
  defaultProviderRequestsPerMinute: 30,
  providerLimits: [],
  developerTimeoutSecs: 1800,
  reviewerTimeoutSecs: 900,
  idleTimeoutSecs: 300,
  storage: { autoCleanup: true, rawLogsDays: 14, trashDays: 7, cacheMaxBytes: 2 * 1024 ** 3 },
  notifications: { enabled: true, onAttention: true, onCompletion: true, onFallback: true },
};

const PROJECT_SETTINGS: ProjectSettings = {
  claudePath: null, codexPath: null, geminiPath: null, qwenPath: null,
  grokPath: null, kimiPath: null, minimaxPath: null, gitPath: null,
  fullAccess: false, resumeSessions: false,
  envDenylist: ["AWS_SECRET_ACCESS_KEY"],
  openai: {}, anthropic: {}, deepseek: {}, grok: {}, minimax: {}, kimi: {},
  apiFallbackProvider: "deepseek_api",
  developerFallbacks: ["claude_code", "codex"],
  reviewerFallbacks: ["codex", "deepseek_api"],
};

const PROJECT_CONFIG_TRUST: ProjectConfigTrust = {
  exists: true,
  path: "/Users/dev/project/.agentflow/project.toml",
  sha256: "3ea76c270b7f8aa6500d05bb8d8c6bb18f36cb23db72a62254d70b93cb61c323",
  trusted: false,
  validationSteps: ["unit tests", "typecheck"],
  extraAllowedCommands: ["bun test"],
  validationCommands: [{ name: "unit tests", argv: ["bun", "test"], timeoutSecs: 600 }],
  environmentAllowlist: ["CI"],
  externalDependencies: [],
  containerImages: [],
  lockEnvironment: false,
  hermetic: false,
  previousApprovedSha256: null,
  changes: [],
  byteOnlyChange: false,
  approvedAt: null,
};

const GIT_COMPATIBILITY: GitCompatibilityReport = {
  repoPath: "/Users/dev/project",
  repositoryIdentity: "8a9e2d7f3c1b",
  shallow: false,
  sparseCheckout: false,
  sparsePatterns: [],
  submodules: [],
  lfsTracked: false,
  lfsAvailable: true,
  sshRemote: true,
  sshAgentAvailable: true,
  networkFilesystem: false,
  caseInsensitive: true,
  caseCollisions: [],
  prunableWorktrees: [],
  repoReadable: true,
  repoWritable: true,
  worktreeRootWritable: true,
  warnings: [],
  blockers: [],
};

const STORAGE: StorageReport = {
  dataDir: "/Users/dev/Library/Application Support/com.agentflow.desktop",
  totalBytes: 348 * 1024 ** 2,
  databaseBytes: 12 * 1024 ** 2,
  taskRuntimeBytes: 210 * 1024 ** 2,
  artifactBytes: 64 * 1024 ** 2,
  logBytes: 48 * 1024 ** 2,
  cacheBytes: 14 * 1024 ** 2,
  trashBytes: 0,
  trashEntries: 0,
  databaseIntegrityOk: true,
  encryptedBackups: 3,
  latestBackupAt: iso(-HOUR),
  runLogsEncrypted: true,
};

const ONBOARDING: OnboardingReport = {
  firstRun: false,
  daemonRunning: true,
  appReady: true,
  workflowReady: true,
  ready: true,
  dataDir: STORAGE.dataDir,
  env: ENV,
  recommendedDeveloper: "claude_code",
  recommendedReviewer: "codex",
  notices: ["预览模式：数据为示例，未连接后端。"],
  storage: STORAGE,
};

/**
 * Codex-shaped output: nested JSON envelopes, a command execution and a final structured
 * summary (05 §11). The main view must show none of the JSON — only the extracted result.
 */
const LOG_LINES: RunLogPage = {
  lines: [
    { ts: iso(-30 * MIN), stream: "stdout", kind: "system", summary: '{"type":"thread.started","thread_id":"01J8"}', text: null },
    {
      ts: iso(-30 * MIN),
      stream: "stdout",
      kind: "assistant_text",
      summary: '{"type":"item.completed","item":{"type":"agent_message","text":"先定位空指针来源，检查 profile 字段是否可能缺失。"}}',
      text: null,
    },
    {
      ts: iso(-29 * MIN),
      stream: "stdout",
      kind: "assistant_text",
      summary: '{"type":"item.completed","item":{"type":"command_execution","command":"bun test","status":"completed"}}',
      text: null,
    },
    { ts: iso(-29 * MIN), stream: "stdout", kind: "tool_use", summary: "Edit /var/folders/xy/T/agentflow/src/auth/login.ts (+9 -2)", text: null },
    { ts: iso(-28 * MIN), stream: "stderr", kind: "tool_result", summary: "warning: 1 unused import", text: null },
    {
      ts: iso(-27 * MIN),
      stream: "stdout",
      kind: "assistant_text",
      summary: '{"type":"item.completed","item":{"type":"agent_message","text":"{\\"summary\\":\\"修复登录空指针并补充单测。\\",\\"changes\\":[\\"src/auth/login.ts 增加可选链兜底\\",\\"新增 login.test.ts 覆盖缺失字段\\"],\\"tests\\":[\\"12 项测试全部通过\\",\\"typecheck 通过\\"],\\"issues\\":[\\"Windows rename 行为还没有真实环境验证\\"],\\"next_action\\":\\"等待人工批准后合并\\"}"}}',
      text: null,
    },
    { ts: iso(-27 * MIN), stream: "stdout", kind: "result", summary: '{"type":"turn.completed"}', text: null },
  ],
  nextFromLine: 7,
  eof: true,
};

const GOVERNANCE: TaskGovernance = {
  manifest: null,
  quality: {
    taskId: "t12", revision: 2, score: 90, grade: "A", passed: true, replay: false,
    checks: [
      { name: "validation", passed: true, weight: 50, detail: "2 validation steps" },
      { name: "independent_review", passed: true, weight: 25, detail: "pass" },
      { name: "high_risk_issues", passed: true, weight: 15, detail: "0 unresolved issues" },
      { name: "control_plane_changes", passed: false, weight: 10, detail: "1 flagged file" },
    ],
    createdAt: iso(-20 * MIN),
  },
  originalQuality: null,
  latestReplay: null,
  budget: DEMO_BUDGET,
  delivery: null,
};

const EXECUTION_NODES: ExecutionNode[] = [];
const QUEUE_STATES = new Map<string, QueueTaskState>();

function demoQueueState(taskId: string): QueueTaskState {
  return QUEUE_STATES.get(taskId) ?? {
    taskId,
    state: "QUEUED",
    paused: false,
    priority: 0,
    position: 1,
    waitingReason: null,
    notBefore: null,
    lastError: null,
    attempts: 0,
    enqueuedAt: iso(-5 * MIN),
    updatedAt: iso(0),
  };
}

for (const taskId of ["t13", "t15"]) {
  QUEUE_STATES.set(taskId, demoQueueState(taskId));
}

// ── 权限代理样本 (06 §9) ──────────────────────────────────────────────
// 覆盖弹窗需要区分的三类：可授权的网络安装、可授权的工作树外读取（高风险，
// 无项目规则按钮）、以及不可授权的系统变更（无允许按钮）。
const PERM_CWD = "/Users/dev/AgentFlow/worktrees/t9";

const PERMISSION_REQUESTS = new Map<string, PermissionRequest[]>([
  [
    "t9",
    [
      {
        id: "perm-net-1",
        projectId: "p1",
        taskId: "t9",
        revision: 3,
        runId: "run-dev-3",
        providerId: "claude",
        role: "developer",
        actionType: "dependency_install",
        summary: "安装依赖 zod 用于校验订单状态",
        reason: "开发 Agent 需要为订单状态机引入 schema 校验库。",
        operation: {
          argv: ["bun", "add", "zod@3.23.8"],
          cwd: PERM_CWD,
          paths: [{ path: `${PERM_CWD}/package.json`, access: "write", outsideWorktree: false }],
          networkDomains: ["registry.npmjs.org:443"],
          environmentNames: ["HOME", "PATH"],
        },
        riskLevel: "medium",
        grantable: true,
        operationSha256: "op-net-1",
        policySha256: "policy-1",
        status: "pending",
        matchedRuleId: null,
        requestCount: 1,
        requestedAt: iso(-90_000),
        expiresAt: iso(4 * MIN),
        decidedAt: null,
        providerResumeToken: null,
      },
      {
        id: "perm-ext-1",
        projectId: "p1",
        taskId: "t9",
        revision: 3,
        runId: "run-dev-3",
        providerId: "claude",
        role: "developer",
        actionType: "external_path",
        summary: "读取工作树外的共享类型定义",
        reason: "Agent 想引用 monorepo 根目录外的一个类型文件。",
        operation: {
          argv: [],
          cwd: PERM_CWD,
          paths: [{ path: "/Users/dev/other-project/shared/types.ts", access: "read", outsideWorktree: true }],
          networkDomains: [],
          environmentNames: [],
        },
        riskLevel: "high",
        grantable: true,
        operationSha256: "op-ext-1",
        policySha256: "policy-1",
        status: "pending",
        matchedRuleId: null,
        requestCount: 2,
        requestedAt: iso(-60_000),
        expiresAt: iso(3 * MIN),
        decidedAt: null,
        providerResumeToken: null,
      },
      {
        id: "perm-sys-1",
        projectId: "p1",
        taskId: "t9",
        revision: 3,
        runId: "run-dev-3",
        providerId: "claude",
        role: "developer",
        actionType: "system_change",
        summary: "使用 sudo 安装系统级构建工具",
        reason: "Agent 试图通过 sudo 修改系统环境——属于不可授权项。",
        operation: {
          argv: ["sudo", "apt-get", "install", "-y", "build-essential"],
          cwd: PERM_CWD,
          paths: [],
          networkDomains: [],
          environmentNames: [],
        },
        riskLevel: "forbidden",
        grantable: false,
        operationSha256: "op-sys-1",
        policySha256: "policy-1",
        status: "pending",
        matchedRuleId: null,
        requestCount: 1,
        requestedAt: iso(-30_000),
        expiresAt: iso(3 * MIN),
        decidedAt: null,
        providerResumeToken: null,
      },
    ],
  ],
]);

const PERMISSION_RULES = new Map<string, PermissionRule[]>([
  [
    "p1",
    [
      {
        id: "rule-test",
        projectId: "p1",
        projectIdentity: "identity-p1",
        providerId: "claude",
        role: "developer",
        actionType: "command_execute",
        operation: { argv: ["bun", "test"], cwd: PERM_CWD, paths: [], networkDomains: [], environmentNames: [] },
        ruleSha256: "rule-sha-test",
        enabled: true,
        createdBy: "human",
        createdAt: iso(-2 * DAY),
        expiresAt: null,
        lastMatchedAt: iso(-15 * MIN),
        revokedAt: null,
      },
      {
        id: "rule-registry",
        projectId: "p1",
        projectIdentity: "identity-p1",
        providerId: "claude",
        role: "developer",
        actionType: "dependency_install",
        operation: { argv: ["bun", "add"], cwd: PERM_CWD, paths: [], networkDomains: ["registry.npmjs.org:443"], environmentNames: [] },
        ruleSha256: "rule-sha-registry",
        enabled: true,
        createdBy: "human",
        createdAt: iso(-1 * DAY),
        expiresAt: iso(6 * DAY),
        lastMatchedAt: null,
        revokedAt: null,
      },
    ],
  ],
]);

function permissionDecide(input: PermissionDecisionInput): PermissionDecision {
  for (const [, list] of PERMISSION_REQUESTS) {
    const req = list.find((r) => r.id === input.requestId);
    if (!req) continue;
    req.status =
      input.decision === "approve" ? "approved" : input.decision === "deny" ? "denied" : "cancelled";
    req.decidedAt = iso(0);
    if (input.decision === "approve" && input.scope === "project_rule") {
      const rules = PERMISSION_RULES.get(req.projectId) ?? [];
      rules.unshift({
        id: `rule-${req.id}`,
        projectId: req.projectId,
        projectIdentity: `identity-${req.projectId}`,
        providerId: req.providerId,
        role: req.role,
        actionType: req.actionType,
        operation: req.operation,
        ruleSha256: `rule-sha-${req.id}`,
        enabled: true,
        createdBy: "human",
        createdAt: iso(0),
        expiresAt: null,
        lastMatchedAt: null,
        revokedAt: null,
      });
      PERMISSION_RULES.set(req.projectId, rules);
    }
    return {
      id: `dec-${req.id}`,
      requestId: req.id,
      operationSha256: input.operationSha256,
      policySha256: input.policySha256,
      decision: input.decision,
      scope: input.scope,
      expiresAt: input.scope === "project_rule" ? null : iso(30 * MIN),
      approvedBy: "human",
      guidance: input.guidance,
      createdAt: iso(0),
      consumedAt: null,
    };
  }
  throw new Error(`unknown permission request ${input.requestId}`);
}

function handle(cmd: string, payload: any): unknown {
  const args = payload?.args ?? {};
  switch (cmd) {
    case "env_check":
    case "env_set_cli_path":
    case "cli_install":
    case "api_credential_set":
    case "api_credential_delete":
      return ENV;
    case "provider_list":
      return PROVIDERS;
    case "onboarding_check":
      return ONBOARDING;
    case "onboarding_complete":
      return null;
    case "project_list":
      return [PROJECT];
    case "project_import":
      return PROJECT;
    case "project_git_compatibility":
    case "project_prune_stale_worktrees":
      return GIT_COMPATIBILITY;
    case "task_list":
      return SUMMARIES;
    case "task_get":
    case "task_start":
    case "task_cancel":
    case "task_resume_with_guidance":
    case "task_force_approve":
    case "task_approve":
    case "task_reject":
    case "task_merge":
    case "task_mark_merged_external":
    case "task_plan_approve":
    case "task_plan_reject":
    case "task_budget_update":
    case "task_delivery_start":
    case "task_delivery_refresh":
    case "task_rollback":
      return DETAILS.get(args.taskId) ?? DETAILS.get("t12");
    case "task_create":
      return DETAILS.get("t12");
    case "queue_task_status":
      return QUEUE_STATES.get(args.taskId) ?? null;
    case "queue_task_pause": {
      const next = { ...demoQueueState(args.taskId), paused: true, position: null, waitingReason: "paused" as const, updatedAt: iso(0) };
      QUEUE_STATES.set(args.taskId, next);
      return next;
    }
    case "queue_task_resume": {
      const next = { ...demoQueueState(args.taskId), paused: false, position: 1, waitingReason: null, updatedAt: iso(0) };
      QUEUE_STATES.set(args.taskId, next);
      return next;
    }
    case "queue_task_priority": {
      const next = { ...demoQueueState(args.taskId), priority: args.priority, updatedAt: iso(0) };
      QUEUE_STATES.set(args.taskId, next);
      return next;
    }
    case "events_list":
      return events(args.taskId ?? "t12");
    case "run_list":
      return runs(args.taskId ?? "t12");
    case "run_log_tail":
      return LOG_LINES;
    case "diff_get":
      return DIFF;
    case "review_get":
      return REVIEW;
    case "task_governance_get":
      return GOVERNANCE;
    case "task_plan_review_context":
      return { taskId: args.taskId, plans: [], latestDiff: null, detectedDeviations: [], deviationPlanId: null, deviationDetectedAt: null };
    case "task_quality_replay":
      return {
        id: "demo-replay", taskId: args.taskId, revision: args.revision ?? 2,
        status: "succeeded", reproducibilityLevel: "fixed_commit", environmentMatch: true,
        drift: [], originalQuality: GOVERNANCE.quality, replayQuality: GOVERNANCE.quality,
        scoreDelta: 0, errorCode: null, errorDetail: null, createdAt: iso(-MIN), finishedAt: iso(0),
      };
    case "execution_node_list":
      return EXECUTION_NODES;
    case "execution_node_upsert":
      return args.node;
    case "execution_node_check":
      return EXECUTION_NODES.find((node) => node.id === args.nodeId) ?? null;
    case "execution_node_delete":
      return null;
    case "settings_get":
    case "settings_update":
      return SETTINGS;
    case "project_settings_get":
    case "project_settings_update":
      return PROJECT_SETTINGS;
    case "project_config_trust_get":
      return PROJECT_CONFIG_TRUST;
    case "project_config_trust_approve":
      return { ...PROJECT_CONFIG_TRUST, trusted: true, approvedAt: iso(0) };
    case "project_config_trust_revoke":
      return { ...PROJECT_CONFIG_TRUST, trusted: false, approvedAt: null };
    case "permission_request_list":
      return PERMISSION_REQUESTS.get(args.taskId) ?? [];
    case "permission_decide":
      return permissionDecide(args.input as PermissionDecisionInput);
    case "permission_rule_list":
      return PERMISSION_RULES.get(args.projectId) ?? [];
    case "permission_rule_revoke": {
      const rules = PERMISSION_RULES.get(args.projectId) ?? [];
      const rule = rules.find((r) => r.id === args.ruleId);
      if (!rule) throw new Error(`unknown rule ${args.ruleId}`);
      rule.revokedAt = iso(0);
      rule.enabled = false;
      return rule;
    }
    case "storage_report":
      return STORAGE;
    case "storage_cleanup":
    case "task_cleanup":
    case "trash_empty":
      return { filesRemoved: 0, bytesReclaimed: 0, tasksTrashed: 0, tasksPurged: 0 };
    case "database_backup_list":
      return [{ path: "/tmp/agentflow-backup.afbak", bytes: 1024, createdAt: iso(-HOUR) }];
    case "database_backup_create":
      return { path: "/tmp/agentflow-backup.afbak", bytes: 1024, createdAt: iso(0) };
    case "database_backup_restore":
      return { restoredBackup: args.path, previousDatabase: "/tmp/pre-restore.db", restartRequired: true };
    case "trash_list":
      return [];
    case "task_restore":
      return SUMMARIES[0];
    case "events_export":
      return {
        path: "/tmp/agentflow-audit-export.jsonl",
        scope: args.taskId ? "task" : "project",
        projectId: args.projectId,
        taskId: args.taskId ?? null,
        eventCount: 42,
        taskCount: args.taskId ? 1 : 3,
        bytes: 4096,
        redacted: true,
        createdAt: iso(0),
      };
    // Tauri event plugin — no-op so listen()/unlisten() resolve cleanly.
    case "plugin:event|listen":
      return 0;
    case "plugin:event|unlisten":
    case "plugin:event|emit":
      return null;
    default:
      return null;
  }
}

export function installTauriDevShim(): void {
  if (!import.meta.env.DEV) return;
  if (typeof window === "undefined") return;
  if ("__TAURI_INTERNALS__" in window) return; // real Tauri host present

  let cbId = 0;
  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    invoke: (cmd: string, payload?: unknown) =>
      new Promise((resolve) => setTimeout(() => resolve(handle(cmd, payload)), 120)),
    transformCallback: (cb?: (v: unknown) => void) => {
      const id = ++cbId;
      (window as any)[`_${id}`] = cb;
      return id;
    },
    convertFileSrc: (p: string) => p,
  };

  // eslint-disable-next-line no-console
  console.info("[AgentFlow] 预览模式：未检测到 Tauri，已启用示例数据 shim（仅 DEV）。");
}
