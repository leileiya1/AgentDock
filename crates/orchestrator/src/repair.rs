impl Orchestrator {
    async fn create_checkpoint(
        &self,
        task: &TaskRow,
        phase: &str,
    ) -> Result<TaskCheckpoint, OrchestratorError> {
        let worktree = required_path(&task.worktree_path)?;
        if !worktree.is_dir() {
            return Err(OrchestratorError::InvalidState("WORKTREE_MISSING".into()));
        }
        let id = Uuid::now_v7().to_string();
        let root = self.task_dir(&task.id).join("checkpoints").join(&id);
        tokio::fs::create_dir_all(&root).await?;
        let commit_sha = self.git.resolve(&worktree, "HEAD").await?;
        let patch = self.git.working_patch(&worktree).await?;
        let (patch_path, patch_sha256) = if patch.is_empty() {
            (None, None)
        } else {
            let path = root.join("residual.patch");
            tokio::fs::write(&path, &patch).await?;
            (
                Some(path.to_string_lossy().into_owned()),
                Some(format!("{:x}", Sha256::digest(&patch))),
            )
        };
        let untracked = self.git.untracked_files(&worktree).await?;
        let untracked_root = root.join("untracked");
        let copied = copy_untracked_snapshot(&worktree, &untracked_root, &untracked).await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO task_checkpoints(id,task_id,revision,phase,commit_sha,patch_path,patch_sha256,untracked_dir,untracked_files,created_at) VALUES(?,?,?,?,?,?,?,?,?,?)")
            .bind(&id)
            .bind(&task.id)
            .bind(task.revision)
            .bind(phase)
            .bind(&commit_sha)
            .bind(patch_path)
            .bind(&patch_sha256)
            .bind((copied > 0).then(|| untracked_root.to_string_lossy().into_owned()))
            .bind(copied)
            .bind(&now)
            .execute(self.store.pool())
            .await?;
        Ok(TaskCheckpoint {
            id,
            revision: task.revision,
            phase: phase.into(),
            commit_sha,
            patch_sha256,
            untracked_files: copied,
            created_at: now,
        })
    }

    pub async fn task_repair_inspect(
        &self,
        task_id: &str,
    ) -> Result<RepairReport, OrchestratorError> {
        let task = self.task(task_id).await?;
        let worktree_exists = task.worktree_path.as_ref().is_some_and(|path| path.is_dir());
        let residual_changes = if worktree_exists {
            self.git
                .has_changes(&required_path(&task.worktree_path)?)
                .await
                .unwrap_or(false)
        } else {
            false
        };
        let checkpoint = self.latest_checkpoint(task_id).await?;
        let mut actions = Vec::new();
        if task.status == TaskStatus::Blocked {
            if !worktree_exists && task.branch.is_some() {
                actions.push(RepairAction::RebuildWorktree);
            }
            if worktree_exists && residual_changes {
                actions.push(RepairAction::ResumeResidual);
            }
            if worktree_exists && checkpoint.is_some() {
                actions.push(RepairAction::ResetToCheckpoint);
            }
        }
        Ok(RepairReport {
            task_id: task.id,
            status: task.status,
            blocked_reason: self.store.task_summary(task_id).await?.blocked_reason,
            worktree_exists,
            residual_changes,
            latest_checkpoint: checkpoint,
            actions,
        })
    }

    pub async fn task_repair_apply(
        &self,
        task_id: &str,
        action: RepairAction,
    ) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if task.status != TaskStatus::Blocked {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        let project = self.project(&task.project_id).await?;
        let worktree = required_path(&task.worktree_path)?;
        self.validate_owned_worktree(&task, &project, &worktree)?;
        match action {
            RepairAction::RebuildWorktree => {
                if worktree.exists() {
                    return Err(OrchestratorError::InvalidState(
                        "worktree already exists".into(),
                    ));
                }
                let branch = task
                    .branch
                    .as_deref()
                    .ok_or_else(|| OrchestratorError::InvalidState("branch missing".into()))?;
                if let Some(parent) = worktree.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                self.git
                    .worktree_add_existing(&project.repo, &worktree, branch)
                    .await?;
                self.git.ensure_agentflow_excluded(&project.repo).await?;
            }
            RepairAction::ResumeResidual => {
                if !worktree.is_dir() || !self.git.has_changes(&worktree).await? {
                    return Err(OrchestratorError::InvalidState(
                        "no residual changes to resume".into(),
                    ));
                }
                self.create_checkpoint(&task, "repair-resume").await?;
            }
            RepairAction::ResetToCheckpoint => {
                if !worktree.is_dir() {
                    return Err(OrchestratorError::InvalidState("WORKTREE_MISSING".into()));
                }
                let checkpoint = self
                    .latest_checkpoint(task_id)
                    .await?
                    .ok_or_else(|| OrchestratorError::InvalidState("checkpoint missing".into()))?;
                self.create_checkpoint(&task, "repair-preserve").await?;
                self.git
                    .reset_owned_worktree(&worktree, &checkpoint.commit_sha)
                    .await?;
            }
        }
        let resume_status = sqlx::query_scalar::<_, String>(
            "SELECT repair_resume_status FROM tasks WHERE id=?",
        )
        .bind(task_id)
        .fetch_optional(self.store.pool())
        .await?
        .and_then(|value| value.parse::<TaskStatus>().ok())
        .filter(|status| repair_runnable_status(*status))
        .unwrap_or(if task.revision == 0 {
            TaskStatus::ReadyForDevelopment
        } else {
            TaskStatus::ReadyForReview
        });
        sqlx::query("UPDATE tasks SET blocked_detail=NULL,repair_resume_status=NULL WHERE id=?")
            .bind(task_id)
            .execute(self.store.pool())
            .await?;
        self.store
            .transition(
                task_id,
                &[TaskStatus::Blocked],
                resume_status,
                None,
                Actor::Human,
                "repair:applied",
                &json!({"action": action}),
            )
            .await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }

    fn validate_owned_worktree(
        &self,
        task: &TaskRow,
        project: &ProjectRow,
        worktree: &Path,
    ) -> Result<(), OrchestratorError> {
        let expected = isolated_worktree_path(project, task);
        // Tasks created before UUID-isolated slots used this legacy location. Continue to repair
        // those exact database-owned paths without accepting arbitrary paths under the root.
        let legacy = project
            .worktree_root
            .join(format!("p{}/t{}", project.seq, task.seq));
        if (worktree != expected && worktree != legacy)
            || worktree
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(OrchestratorError::InvalidState(
                "repair refused a non-AgentFlow worktree path".into(),
            ));
        }
        Ok(())
    }

    async fn latest_checkpoint(
        &self,
        task_id: &str,
    ) -> Result<Option<TaskCheckpoint>, OrchestratorError> {
        let row = sqlx::query("SELECT id,revision,phase,commit_sha,patch_sha256,untracked_files,created_at FROM task_checkpoints WHERE task_id=? ORDER BY created_at DESC,id DESC LIMIT 1")
            .bind(task_id)
            .fetch_optional(self.store.pool())
            .await?;
        Ok(row.map(|row| TaskCheckpoint {
            id: row.get("id"),
            revision: row.get("revision"),
            phase: row.get("phase"),
            commit_sha: row.get("commit_sha"),
            patch_sha256: row.get("patch_sha256"),
            untracked_files: row.get("untracked_files"),
            created_at: row.get("created_at"),
        }))
    }
}

fn repair_runnable_status(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::ReadyForDevelopment
            | TaskStatus::ReadyForRevision
            | TaskStatus::Validating
            | TaskStatus::ReadyForReview
    )
}

async fn copy_untracked_snapshot(
    worktree: &Path,
    target: &Path,
    paths: &[PathBuf],
) -> Result<i64, OrchestratorError> {
    let mut copied = 0_i64;
    let mut bytes = 0_u64;
    for relative in paths.iter().take(200) {
        if relative.is_absolute()
            || relative.components().any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            continue;
        }
        let source = worktree.join(relative);
        let metadata = tokio::fs::symlink_metadata(&source).await?;
        if !metadata.is_file() || bytes.saturating_add(metadata.len()) > 20 * 1024 * 1024 {
            continue;
        }
        let destination = target.join(relative);
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(source, destination).await?;
        bytes += metadata.len();
        copied += 1;
    }
    Ok(copied)
}
