CREATE TABLE project_config_trust (
  project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
  config_sha256 TEXT NOT NULL,
  approved_at TEXT NOT NULL
);
