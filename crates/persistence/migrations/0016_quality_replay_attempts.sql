CREATE TABLE quality_replay_attempts (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  revision INTEGER NOT NULL,
  status TEXT NOT NULL,
  attempt_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX idx_quality_replay_latest
  ON quality_replay_attempts(task_id, revision, created_at DESC);
