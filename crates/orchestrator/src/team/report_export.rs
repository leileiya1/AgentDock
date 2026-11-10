use super::*;
impl Orchestrator {
    /// Local, private snapshot export. It never decrypts a key into the package, overwrites a
    /// chosen directory, accepts a criterion, or changes the task's execution state.
    pub async fn team_report_export(
        &self,
        input: TeamReportExportInput,
    ) -> Result<TeamReportExport, OrchestratorError> {
        let view = self
            .team_get(&input.task_id)
            .await?
            .ok_or_else(|| err("not a team task"))?;
        let parent = Path::new(&input.parent_directory);
        if !parent.is_absolute() {
            return Err(err("choose an absolute existing parent directory"));
        }
        let parent = tokio::fs::canonicalize(parent).await?;
        if !parent.is_dir() {
            return Err(err("export parent must be a directory"));
        }
        let dir = parent.join(format!(
            "agentflow-team-{}-{}",
            view.task_id,
            Uuid::now_v7()
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&dir)?;
        let result = self.team_write_export(&view, &dir).await;
        if result.is_err() {
            let _ = tokio::fs::remove_dir_all(&dir).await;
        }
        result
    }
    async fn team_write_export(
        &self,
        view: &TeamView,
        dir: &Path,
    ) -> Result<TeamReportExport, OrchestratorError> {
        let report = serde_json::to_vec_pretty(view).map_err(|e| err(e.to_string()))?;
        write_private(&dir.join("team-report.json"), &report).await?;
        tokio::fs::create_dir(dir.join("evidence")).await?;
        let mut records = Vec::new();
        let mut seen = HashSet::new();
        let mut total_bytes = report.len();
        for c in &view.criteria {
            for evidence in &c.evidence {
                if !seen.insert(&evidence.id) {
                    continue;
                }
                // Same task containment, original content verification and redaction as the UI.
                let display = self.team_evidence_read(&view.task_id, &evidence.id).await?;
                total_bytes += display.len();
                if total_bytes > 50 * 1024 * 1024 {
                    return Err(err(
                        "report exceeds 50 MiB export limit; no partial export was retained",
                    ));
                }
                let relative = format!("evidence/{}.txt", evidence.id);
                write_private(&dir.join(&relative), display.as_bytes()).await?;
                records.push(json!({"path":relative,"evidenceId":evidence.id,"criterionId":c.criterion.id,"candidateId":evidence.candidate_id,"checkId":evidence.check_id,"status":evidence.status,"originalSha256":evidence.raw_sha256,"exportedDisplaySha256":hex_digest(display.as_bytes()),"format":"verified, redacted and bounded display; original digest refers to the full original record"}));
            }
        }
        let manifest = json!({"schemaVersion":1,"snapshotTakenAt":Utc::now().to_rfc3339(),"taskId":view.task_id,"phase":view.phase,"planRevision":view.plan_revision,"candidateId":view.candidate.as_ref().map(|c|&c.id),"report":{"path":"team-report.json","sha256":hex_digest(&report)},"evidence":records,"limitations":view.limitations,"interpretation":"A snapshot of backend facts, including failures/history. Export itself does not accept or deliver a task. Evidence displays may be redacted or truncated; both original and exported display digests are distinct."});
        let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| err(e.to_string()))?;
        write_private(&dir.join("manifest.json"), &bytes).await?;
        Ok(TeamReportExport {
            directory: dir.to_string_lossy().into_owned(),
            candidate_id: view.candidate.as_ref().map(|c| c.id.clone()),
            evidence_count: records.len() as u32,
            report_sha256: hex_digest(&report),
            manifest_sha256: hex_digest(&bytes),
        })
    }
}
async fn write_private(path: &Path, bytes: &[u8]) -> Result<(), OrchestratorError> {
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    let mut file = options.open(path).await?;
    use tokio::io::AsyncWriteExt;
    file.write_all(bytes).await?;
    file.sync_all().await?;
    Ok(())
}
