CREATE TABLE task_operations (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  phase TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'RUNNING',
  payload_json TEXT NOT NULL,
  last_error TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_task_operation_running
  ON task_operations(task_id, kind) WHERE status='RUNNING';
CREATE INDEX idx_task_operation_recovery ON task_operations(status, kind, updated_at);
