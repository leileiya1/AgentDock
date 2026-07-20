ALTER TABLE tasks ADD COLUMN deleted_at TEXT;

CREATE INDEX idx_tasks_visible
  ON tasks(project_id, seq)
  WHERE deleted_at IS NULL;

CREATE TABLE trash_items (
  task_id TEXT PRIMARY KEY REFERENCES tasks(id),
  original_path TEXT NOT NULL,
  trashed_path TEXT NOT NULL,
  bytes INTEGER NOT NULL DEFAULT 0,
  trashed_at TEXT NOT NULL,
  purge_after TEXT NOT NULL
);

CREATE INDEX idx_trash_items_purge
  ON trash_items(purge_after);
