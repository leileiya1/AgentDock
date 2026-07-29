CREATE TABLE task_acceptance_criteria (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('build', 'test', 'behavior', 'manual')),
  text TEXT NOT NULL CHECK (length(trim(text)) BETWEEN 1 AND 500),
  created_at TEXT NOT NULL,
  UNIQUE(task_id, position)
);

CREATE INDEX idx_task_acceptance_criteria_task
  ON task_acceptance_criteria(task_id, position);
