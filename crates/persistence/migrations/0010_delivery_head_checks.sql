ALTER TABLE delivery_records ADD COLUMN approved_commit_sha TEXT;
ALTER TABLE delivery_records ADD COLUMN observed_head_sha TEXT;
ALTER TABLE delivery_records ADD COLUMN head_branch TEXT;
ALTER TABLE delivery_records ADD COLUMN base_branch TEXT;
ALTER TABLE delivery_records ADD COLUMN required_checks_json TEXT;
