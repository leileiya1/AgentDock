ALTER TABLE execution_nodes
  ADD COLUMN diagnostics_json TEXT NOT NULL DEFAULT '[]';
