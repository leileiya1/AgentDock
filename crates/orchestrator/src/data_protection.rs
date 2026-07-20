const SENSITIVE_RUN_FILES: &[&str] = &[
    "input.md",
    "stdout.log",
    "stderr.log",
    "agent-events.jsonl",
    "last-message.json",
    "result.json",
    "provider-telemetry.json",
];

impl Orchestrator {
    /// Seal completed Provider material in place. Database metadata remains queryable,
    /// while prompts, model output and raw event streams require the local data key.
    async fn protect_run_files(&self, run_dir: &Path) -> Result<(), OrchestratorError> {
        for name in SENSITIVE_RUN_FILES {
            self.store.protect_file(&run_dir.join(name)).await?;
        }
        Ok(())
    }

    async fn read_run_file(&self, path: &Path) -> Result<Vec<u8>, OrchestratorError> {
        self.store.read_protected_file(path).await.map_err(Into::into)
    }
}
