ALTER TABLE agent_runs ADD COLUMN recovery_state TEXT;
ALTER TABLE agent_runs ADD COLUMN adopted_at TEXT;

CREATE INDEX idx_agent_runs_adoption
  ON agent_runs(status, recovery_state, task_id);
