PRAGMA foreign_keys = ON;

CREATE TABLE projects (
  id TEXT PRIMARY KEY, seq INTEGER NOT NULL UNIQUE, name TEXT NOT NULL,
  repo_path TEXT NOT NULL UNIQUE, default_branch TEXT NOT NULL, worktree_root TEXT NOT NULL,
  settings_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE tasks (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), seq INTEGER NOT NULL,
  title TEXT NOT NULL, description TEXT NOT NULL, status TEXT NOT NULL, blocked_reason TEXT,
  blocked_detail TEXT, developer_agent TEXT NOT NULL, reviewer_agent TEXT NOT NULL,
  target_branch TEXT NOT NULL, base_commit TEXT, branch TEXT, worktree_path TEXT,
  current_revision INTEGER NOT NULL DEFAULT 0, max_revisions INTEGER NOT NULL DEFAULT 3,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, UNIQUE(project_id, seq)
);
CREATE TABLE task_revisions (
  id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id), revision INTEGER NOT NULL,
  commit_sha TEXT, diff_stat_json TEXT, developer_run_id TEXT, created_at TEXT NOT NULL,
  UNIQUE(task_id, revision)
);
CREATE TABLE agent_runs (
  id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id), revision INTEGER NOT NULL,
  role TEXT NOT NULL, agent TEXT, status TEXT NOT NULL, child_pid INTEGER, child_started_at TEXT,
  exit_code INTEGER, session_id TEXT, cost_usd REAL, tokens_in INTEGER, tokens_out INTEGER,
  run_dir TEXT NOT NULL, timeout_secs INTEGER NOT NULL, idle_timeout_secs INTEGER NOT NULL,
  started_at TEXT, finished_at TEXT, created_at TEXT NOT NULL
);
CREATE INDEX idx_runs_task ON agent_runs(task_id, created_at);
CREATE UNIQUE INDEX idx_one_active_run_per_task ON agent_runs(task_id) WHERE status = 'RUNNING';
CREATE TABLE reviews (
  id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id), revision INTEGER NOT NULL,
  run_id TEXT NOT NULL REFERENCES agent_runs(id), commit_sha TEXT NOT NULL, decision TEXT NOT NULL,
  summary TEXT, raw_path TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE review_issues (
  id TEXT PRIMARY KEY, review_id TEXT NOT NULL REFERENCES reviews(id), severity TEXT NOT NULL,
  file TEXT, line_start INTEGER, line_end INTEGER, title TEXT NOT NULL, description TEXT,
  suggested_action TEXT, resolved INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE approvals (
  id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id), revision INTEGER NOT NULL,
  commit_sha TEXT NOT NULL, diff_sha256 TEXT NOT NULL, action TEXT NOT NULL, reason TEXT,
  created_at TEXT NOT NULL
);
CREATE TABLE events (
  id INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT, run_id TEXT, revision INTEGER,
  actor TEXT NOT NULL, event_type TEXT NOT NULL, payload_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE INDEX idx_events_task ON events(task_id, id);
CREATE TABLE artifacts (
  id TEXT PRIMARY KEY, task_id TEXT NOT NULL, revision INTEGER, kind TEXT NOT NULL,
  path TEXT NOT NULL, sha256 TEXT, created_at TEXT NOT NULL
);
CREATE TABLE settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL);
