-- Resume credentials are secrets, not audit facts. Invalidate every legacy plaintext value and
-- retain only opaque Keychain references from this migration onward.
ALTER TABLE agent_runs ADD COLUMN session_secret_ref TEXT;
ALTER TABLE permission_requests ADD COLUMN provider_resume_secret_ref TEXT;

UPDATE agent_runs SET session_id = NULL WHERE session_id IS NOT NULL;
UPDATE permission_requests SET provider_resume_token = NULL WHERE provider_resume_token IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_agent_runs_resume_secret
  ON agent_runs(task_id, revision, role, agent, status)
  WHERE session_secret_ref IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_permission_requests_resume_secret
  ON permission_requests(task_id, provider_id, role, status, requested_at)
  WHERE provider_resume_secret_ref IS NOT NULL;
