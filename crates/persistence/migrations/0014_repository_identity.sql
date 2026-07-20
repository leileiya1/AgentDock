ALTER TABLE projects ADD COLUMN repo_identity TEXT;

CREATE UNIQUE INDEX idx_projects_repo_identity
  ON projects(repo_identity) WHERE repo_identity IS NOT NULL;
