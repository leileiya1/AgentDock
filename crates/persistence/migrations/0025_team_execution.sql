-- Additive opt-in mode; legacy tasks acquire no invented team/evidence rows.
CREATE TABLE team_tasks (
 task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
 phase TEXT NOT NULL, goal TEXT NOT NULL, config_json TEXT NOT NULL,
 requirements_json TEXT NOT NULL, project_settings_json TEXT NOT NULL, plan_revision INTEGER NOT NULL DEFAULT 0,
 current_candidate_id TEXT, attempts_used INTEGER NOT NULL DEFAULT 0,
 reworks_used INTEGER NOT NULL DEFAULT 0, detail TEXT, report_path TEXT,
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE team_plans (
 task_id TEXT NOT NULL REFERENCES team_tasks(task_id) ON DELETE CASCADE,
 revision INTEGER NOT NULL, hash TEXT NOT NULL, plan_json TEXT NOT NULL,
 reason TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(task_id,revision)
);
CREATE TABLE team_work_items (
 task_id TEXT NOT NULL REFERENCES team_tasks(task_id) ON DELETE CASCADE,
 id TEXT NOT NULL, spec_json TEXT NOT NULL, spec_hash TEXT NOT NULL,
 state TEXT NOT NULL, epoch INTEGER NOT NULL DEFAULT 0, artifact_id TEXT, detail TEXT,
 PRIMARY KEY(task_id,id)
);
CREATE TABLE team_work_history (
 id INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT NOT NULL,
 work_item_id TEXT NOT NULL, spec_json TEXT NOT NULL, spec_hash TEXT NOT NULL,
 artifact_id TEXT, plan_revision INTEGER NOT NULL, created_at TEXT NOT NULL
);
ALTER TABLE agent_runs ADD COLUMN team_work_item_id TEXT;
ALTER TABLE agent_runs ADD COLUMN team_member_id TEXT;
DROP INDEX idx_one_active_run_per_task;
CREATE UNIQUE INDEX idx_one_active_run_per_task ON agent_runs(task_id)
 WHERE status='RUNNING' AND team_work_item_id IS NULL;
CREATE UNIQUE INDEX idx_team_one_active_work ON agent_runs(task_id,team_work_item_id)
 WHERE status='RUNNING' AND team_work_item_id IS NOT NULL;
CREATE UNIQUE INDEX idx_team_one_active_seat ON agent_runs(task_id,team_member_id)
 WHERE status='RUNNING' AND team_member_id IS NOT NULL;
CREATE TABLE team_attempt_contexts (
 run_id TEXT PRIMARY KEY REFERENCES agent_runs(id), identity_json TEXT NOT NULL,
 spec_json TEXT NOT NULL, input_json TEXT NOT NULL, worktree TEXT, input_base_sha TEXT,
 failure_class TEXT, cancel_requested INTEGER NOT NULL DEFAULT 0,
 lease_until TEXT NOT NULL, start_intent TEXT NOT NULL DEFAULT 'reserved'
);
CREATE TABLE team_artifacts (
 id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES team_tasks(task_id),
 work_item_id TEXT NOT NULL, attempt_id TEXT NOT NULL REFERENCES agent_runs(id),
 artifact_json TEXT NOT NULL, patch BLOB NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE team_candidates (
 id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES team_tasks(task_id),
 candidate_json TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE team_evidence (
 id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES team_tasks(task_id),
 candidate_id TEXT NOT NULL REFERENCES team_candidates(id), criterion_id TEXT NOT NULL,
 attempt_id TEXT NOT NULL REFERENCES agent_runs(id), evidence_json TEXT NOT NULL,
 created_at TEXT NOT NULL
);
CREATE INDEX idx_team_evidence_candidate ON team_evidence(task_id,candidate_id,criterion_id);
CREATE TABLE team_audits (
 id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES team_tasks(task_id),
 kind TEXT NOT NULL, plan_hash TEXT NOT NULL, candidate_id TEXT,
 member_id TEXT NOT NULL, provider TEXT NOT NULL, attempt_id TEXT NOT NULL REFERENCES agent_runs(id),
 approved INTEGER NOT NULL, detail TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE team_issues (
 id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES team_tasks(task_id),
 candidate_id TEXT, criterion_id TEXT, owner_work_item_id TEXT NOT NULL,
 detail TEXT NOT NULL, resolved_by_candidate_id TEXT, created_at TEXT NOT NULL
);
-- Deduplication identity is independent of UI event cursor and timestamp.
ALTER TABLE events ADD COLUMN team_event_key TEXT;
CREATE UNIQUE INDEX idx_team_event_key ON events(team_event_key) WHERE team_event_key IS NOT NULL;
CREATE TABLE team_project_locks (
 project_id TEXT PRIMARY KEY REFERENCES projects(id), operation_id TEXT NOT NULL UNIQUE,
 task_id TEXT NOT NULL REFERENCES tasks(id), kind TEXT NOT NULL
);
