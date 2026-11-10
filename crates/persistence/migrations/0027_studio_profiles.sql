-- Town / studio customisation. Additive only; owned by the desktop "town" view.
CREATE TABLE project_appearance (
  project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
  kind TEXT,
  roof TEXT,
  sign TEXT,
  yard_json TEXT NOT NULL DEFAULT '[]',
  updated_at TEXT NOT NULL
);

CREATE TABLE project_members (
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  agent TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('developer', 'reviewer', 'both')),
  position INTEGER NOT NULL,
  added_at TEXT NOT NULL,
  PRIMARY KEY (project_id, agent)
);
CREATE INDEX project_members_order ON project_members(project_id, position);

CREATE TABLE agent_profiles (
  agent TEXT PRIMARY KEY,
  nickname TEXT,
  color TEXT,
  accessory TEXT,
  catchphrase TEXT,
  updated_at TEXT NOT NULL
);
