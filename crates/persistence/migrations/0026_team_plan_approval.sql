-- Approval metadata is additive; never rewrite the team schema after a database used it.
ALTER TABLE team_plans ADD COLUMN approved_at TEXT;
