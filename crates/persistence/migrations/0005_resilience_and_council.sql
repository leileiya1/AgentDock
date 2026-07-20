ALTER TABLE tasks ADD COLUMN repair_resume_status TEXT;

CREATE TABLE task_checkpoints (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id),
  revision INTEGER NOT NULL,
  phase TEXT NOT NULL,
  commit_sha TEXT NOT NULL,
  patch_path TEXT,
  patch_sha256 TEXT,
  untracked_dir TEXT,
  untracked_files INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_checkpoints_task ON task_checkpoints(task_id, created_at DESC);

ALTER TABLE reviews ADD COLUMN reviewer_agent TEXT;
ALTER TABLE reviews ADD COLUMN is_aggregate INTEGER NOT NULL DEFAULT 0;
ALTER TABLE reviews ADD COLUMN member_review_ids_json TEXT;
ALTER TABLE reviews ADD COLUMN reviewer_agents_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE review_issues ADD COLUMN reported_by_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE review_issues ADD COLUMN agreement_count INTEGER NOT NULL DEFAULT 1;
