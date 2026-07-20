CREATE TABLE daemon_queue (
  task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
  state TEXT NOT NULL DEFAULT 'QUEUED',
  attempts INTEGER NOT NULL DEFAULT 0,
  not_before TEXT,
  last_error TEXT,
  enqueued_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_daemon_queue_ready
  ON daemon_queue(state, not_before, enqueued_at);
