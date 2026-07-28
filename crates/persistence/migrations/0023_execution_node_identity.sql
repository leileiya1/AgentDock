-- Store only the local path to an SSH identity. Private-key bytes remain owned by OpenSSH/Lima
-- and are never copied into AgentDock's database, logs, backups, or task archives.
ALTER TABLE execution_nodes ADD COLUMN identity_file TEXT;
