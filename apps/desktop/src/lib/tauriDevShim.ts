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
  Project,
  ProjectConfigTrust,
  ProjectSettings,
  ProviderDescriptor,
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
});

const ENV: EnvReport = {
  git: okTool("/usr/bin/git", "2.43.0"),
  claudeCode: okTool("/opt/homebrew/bin/claude", "1.0.0"),
  codex: okTool("/opt/homebrew/bin/codex", "0.9.2"),
  geminiCli: missingTool(),
  qwenCode: missingTool(),
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
  summary(9, "重构订单状态机", "BLOCKED", "max_revisions", 3, -1 * HOUR),
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

const DEMO_BUDGET = {
  tokensUsed: 32_840,
  costUsd: 0.42,
  timeUsedSecs: 480,
  tokenBudget: 500_000,
  costBudgetUsd: 25,
  timeBudgetSecs: 7_200,
  tokensKnown: true,
  costKnown: true,
  unknownTokenRuns: 0,
  unknownCostRuns: 0,
  tokensReserved: 0,
  costReservedUsd: 0,
  tokenEnforcement: "hard" as const,
  costEnforcement: "hard" as const,
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
    blockedDetail:
      s.status === "BLOCKED"
        ? "连续 3 轮审查仍要求返工，已达上限。"
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

function events(taskId: string): TaskEvent[] {
  const base: Array<[string, TaskEvent["actor"], number | null, number]> = [
    ["task_created", "human", null, -60],
    ["develop_started", "agent", 1, -55],
    ["develop_completed", "agent", 1, -50],
    ["validate_completed", "orchestrator", 1, -48],
    ["review_request_changes", "agent", 1, -46],
    ["revise_started", "agent", 2, -30],
    ["develop_completed", "agent", 2, -26],
    ["validate_completed", "orchestrator", 2, -24],
    ["review_passed", "agent", 2, -22],
    ["waiting_for_human_approval", "orchestrator", 2, -20],
  ];
  return base.map(([eventType, actor, revision, min], i) => ({
    id: i + 1,
    taskId,
    runId: revision != null ? `run-${taskId}-${revision}-${eventType}` : null,
    revision,
    actor,
    eventType,
    payload: eventType.includes("develop_completed")
      ? { summary: `第 ${revision} 轮：已修复空指针并补充 2 个单测。` }
      : {},
    createdAt: iso(min * MIN),
  }));
}

function runs(taskId: string): RunSummary[] {
  const mk = (rev: number, role: RunSummary["role"], agent: RunSummary["agent"], status: RunSummary["status"], min: number): RunSummary => ({
    id: `run-${taskId}-${rev}-${role}`,
    taskId,
    revision: rev,
    role,
    agent,
    status,
    exitCode: status === "SUCCEEDED" ? 0 : status === "RUNNING" ? null : 1,
    costUsd: 0.12,
    tokensIn: 5_000,
    tokensOut: 1_800,
    startedAt: iso(min * MIN),
    finishedAt: status === "RUNNING" ? null : iso((min + 3) * MIN),
  });
  return [
    mk(1, "developer", "claude_code", "SUCCEEDED", -55),
    mk(1, "validator", null, "SUCCEEDED", -50),
    mk(1, "reviewer", "codex", "SUCCEEDED", -46),
    mk(2, "developer", "claude_code", "SUCCEEDED", -30),
    mk(2, "reviewer", "codex", "SUCCEEDED", -22),
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
  ],
};

const REVIEW: Review = {
  id: "rev-1",
  revision: 2,
  commitSha: "9f8e7d6c5b4a2",
  decision: "pass",
  summary: "改动聚焦、兜底合理，单测覆盖到了缺失字段的分支。放行。",
  reviewerAgents: ["claude_code", "deepseek_api"],
  issues: [
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
  ready: true,
  dataDir: STORAGE.dataDir,
  env: ENV,
  recommendedDeveloper: "claude_code",
  recommendedReviewer: "codex",
  notices: ["预览模式：数据为示例，未连接后端。"],
  storage: STORAGE,
};

const LOG_LINES: RunLogPage = {
  lines: [
    { ts: iso(-30 * MIN), stream: "stdout", kind: "system", summary: "会话开始 · model claude-opus-4-8", text: null },
    { ts: iso(-30 * MIN), stream: "stdout", kind: "assistant_text", summary: "先定位空指针来源，检查 profile 字段。", text: "先定位空指针来源，检查 profile 字段是否可能缺失，再决定兜底策略。" },
    { ts: iso(-29 * MIN), stream: "stdout", kind: "tool_use", summary: "Read src/auth/login.ts", text: null },
    { ts: iso(-29 * MIN), stream: "stdout", kind: "tool_use", summary: "Edit src/auth/login.ts (+9 -2)", text: null },
    { ts: iso(-28 * MIN), stream: "stdout", kind: "tool_use", summary: "Bash bun test", text: null },
    { ts: iso(-27 * MIN), stream: "stdout", kind: "result", summary: "完成：修复空指针，新增 2 个单测，全部通过。", text: null },
  ],
  nextFromLine: 6,
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
  budget: DEMO_BUDGET,
  delivery: null,
};

const EXECUTION_NODES: ExecutionNode[] = [];

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
    case "queue_task_pause":
      return { taskId: args.taskId, paused: true, priority: null };
    case "queue_task_resume":
      return { taskId: args.taskId, paused: false, priority: null };
    case "queue_task_priority":
      return { taskId: args.taskId, paused: null, priority: args.priority };
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
    case "task_quality_replay":
      return GOVERNANCE.quality;
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
      return { path: "/tmp/agentflow-export.jsonl" };
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
