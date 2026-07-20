ALTER TABLE task_plans ADD COLUMN allowed_paths_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE task_plans ADD COLUMN plan_sha256 TEXT;
