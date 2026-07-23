CREATE TABLE permission_requests (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  revision INTEGER NOT NULL,
  run_id TEXT,
  provider_id TEXT NOT NULL,
  role TEXT NOT NULL,
  action_type TEXT NOT NULL,
  summary TEXT NOT NULL,
  reason TEXT NOT NULL,
  operation_json TEXT NOT NULL,
  risk_level TEXT NOT NULL CHECK(risk_level IN ('low','medium','high','forbidden')),
  grantable INTEGER NOT NULL CHECK(grantable IN (0,1)),
  operation_sha256 TEXT NOT NULL,
  policy_sha256 TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('pending','approved','denied','cancelled','expired')),
  resume_status TEXT NOT NULL,
  retry_revision INTEGER NOT NULL,
  matched_rule_id TEXT,
  request_count INTEGER NOT NULL DEFAULT 1,
  requested_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  decided_at TEXT,
  provider_resume_token TEXT
);
CREATE INDEX idx_permission_requests_task_status ON permission_requests(task_id,status,requested_at DESC);
CREATE INDEX idx_permission_requests_operation ON permission_requests(task_id,operation_sha256,policy_sha256);

CREATE TABLE permission_decisions (
  id TEXT PRIMARY KEY,
  request_id TEXT NOT NULL REFERENCES permission_requests(id) ON DELETE CASCADE,
  operation_sha256 TEXT NOT NULL,
  policy_sha256 TEXT NOT NULL,
  decision TEXT NOT NULL CHECK(decision IN ('approve','deny','cancel_task')),
  scope TEXT NOT NULL CHECK(scope IN ('once','task','project_rule')),
  expires_at TEXT,
  approved_by TEXT NOT NULL CHECK(approved_by='human'),
  guidance TEXT,
  created_at TEXT NOT NULL,
  consumed_at TEXT
);
CREATE INDEX idx_permission_decisions_request ON permission_decisions(request_id,created_at DESC);

CREATE TABLE permission_rules (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  project_identity TEXT NOT NULL,
  provider_id TEXT NOT NULL,
  role TEXT NOT NULL,
  action_type TEXT NOT NULL,
  operation_json TEXT NOT NULL,
  rule_sha256 TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
  created_by TEXT NOT NULL CHECK(created_by='human'),
  created_at TEXT NOT NULL,
  expires_at TEXT,
  last_matched_at TEXT,
  revoked_at TEXT
);
CREATE UNIQUE INDEX idx_permission_rules_active_sha ON permission_rules(project_id,rule_sha256) WHERE enabled=1;

-- Permanent host-wide access was an unsafe legacy setting. Preserve the schema for old clients,
-- but invalidate every stored value during migration; no rule is silently created from it.
UPDATE projects
SET settings_json = json_set(settings_json, '$.fullAccess', json('false'))
WHERE json_extract(settings_json, '$.fullAccess') = 1;
