-- A protected remote validation must opt in explicitly. Existing execution nodes retain their
-- prior behavior until the operator enables and successfully diagnoses the network boundary.
ALTER TABLE execution_nodes ADD COLUMN deny_network INTEGER NOT NULL DEFAULT 0;
