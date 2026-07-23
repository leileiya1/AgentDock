-- §24: record when council members disagreed on how serious an issue is (one flagged it
-- critical/high while another rated it medium/low). Purely informational — the aggregate decision
-- still uses the highest severity; this only lets the UI surface the disagreement to the human.
ALTER TABLE review_issues ADD COLUMN severity_disagreement INTEGER NOT NULL DEFAULT 0;
