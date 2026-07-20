ALTER TABLE agent_runs ADD COLUMN token_budget_mode TEXT NOT NULL DEFAULT 'soft';
ALTER TABLE agent_runs ADD COLUMN cost_budget_mode TEXT NOT NULL DEFAULT 'soft';
ALTER TABLE agent_runs ADD COLUMN reserved_tokens INTEGER;
ALTER TABLE agent_runs ADD COLUMN reserved_cost_usd REAL;

CREATE INDEX idx_runs_budget_reservations
ON agent_runs(task_id, status, reserved_tokens, reserved_cost_usd);
