ALTER TABLE tasks ADD COLUMN api_egress_approved_at TEXT;

ALTER TABLE review_issues ADD COLUMN resolved_at TEXT;
ALTER TABLE review_issues ADD COLUMN resolved_by_revision INTEGER;

CREATE INDEX idx_review_issues_unresolved ON review_issues(review_id, resolved);
