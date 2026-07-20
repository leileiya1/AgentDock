CREATE TABLE task_policies (
  task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
  require_plan_approval INTEGER NOT NULL DEFAULT 1,
  token_budget INTEGER,
  cost_budget_usd REAL,
  time_budget_secs INTEGER,
  minimum_quality_score INTEGER NOT NULL DEFAULT 70,
  delivery_mode TEXT NOT NULL DEFAULT 'local_merge',
  execution_node_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Preserve the behavior of tasks created before the plan gate existed.
INSERT INTO task_policies(
  task_id, require_plan_approval, token_budget, cost_budget_usd,
  time_budget_secs, minimum_quality_score, delivery_mode, created_at, updated_at
)
SELECT id, 0, 500000, 25.0, 7200, 70, 'local_merge', created_at, updated_at FROM tasks;

CREATE TABLE task_plans (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  version INTEGER NOT NULL,
  status TEXT NOT NULL,
  summary TEXT NOT NULL,
  steps_json TEXT NOT NULL,
  risks_json TEXT NOT NULL,
  rejection_reason TEXT,
  created_at TEXT NOT NULL,
  approved_at TEXT,
  UNIQUE(task_id, version)
);
CREATE INDEX idx_task_plans_latest ON task_plans(task_id, version DESC);

CREATE TABLE reproducibility_manifests (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  revision INTEGER NOT NULL,
  commit_sha TEXT NOT NULL,
  manifest_sha256 TEXT NOT NULL,
  manifest_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(task_id, revision)
);

CREATE TABLE quality_evaluations (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  revision INTEGER NOT NULL,
  score INTEGER NOT NULL,
  passed INTEGER NOT NULL,
  replay INTEGER NOT NULL DEFAULT 0,
  evaluation_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_quality_latest ON quality_evaluations(task_id, revision, created_at DESC);

CREATE TABLE delivery_records (
  task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
  mode TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'pending',
  remote_url TEXT,
  request_number INTEGER,
  ci_status TEXT,
  merge_commit TEXT,
  pre_merge_commit TEXT,
  rollback_commit TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE execution_nodes (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  host TEXT NOT NULL,
  port INTEGER NOT NULL DEFAULT 22,
  username TEXT NOT NULL,
  work_root TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  status TEXT NOT NULL DEFAULT 'unknown',
  platform TEXT,
  git_version TEXT,
  problem TEXT,
  last_checked_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

ALTER TABLE agent_runs ADD COLUMN execution_node_id TEXT;
