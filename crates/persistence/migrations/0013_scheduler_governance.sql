ALTER TABLE task_policies ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;
ALTER TABLE daemon_queue ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;
ALTER TABLE daemon_queue ADD COLUMN paused INTEGER NOT NULL DEFAULT 0;

CREATE TABLE provider_slots (
  provider TEXT NOT NULL,
  account TEXT NOT NULL,
  slot INTEGER NOT NULL,
  run_id TEXT NOT NULL UNIQUE,
  acquired_at TEXT NOT NULL,
  PRIMARY KEY(provider, account, slot)
);

CREATE TABLE provider_dispatch_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  provider TEXT NOT NULL,
  account TEXT NOT NULL,
  dispatched_at TEXT NOT NULL
);

CREATE INDEX idx_provider_dispatch_history_window
  ON provider_dispatch_history(provider, account, dispatched_at);

CREATE INDEX idx_daemon_queue_priority
  ON daemon_queue(state, paused, priority DESC, not_before, enqueued_at);
